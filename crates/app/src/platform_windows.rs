use std::{mem::size_of, ptr::null_mut};

use anyhow::{Context, Result, bail};
use image::{DynamicImage, RgbaImage};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows_sys::Win32::{
    Foundation::CloseHandle,
    Graphics::{
        Dwm::{DwmFlush, DwmIsCompositionEnabled},
        Gdi::*,
    },
    System::Threading::{
        OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
        QueryFullProcessImageNameW,
    },
    UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowDisplayAffinity, GetWindowTextW, GetWindowThreadProcessId,
        SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE, WDA_NONE,
    },
};

use super::{Display, Source};

pub fn foreground_source() -> Option<Source> {
    let mut title = [0_u16; 512];
    let mut image = [0_u16; 1024];
    let mut image_len = image.len() as u32;
    let mut pid = 0_u32;
    // SAFETY: every output pointer refers to a live, writable stack buffer whose
    // capacity is passed alongside it; the process handle is closed exactly once.
    let (title_len, has_image) = unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return None;
        }
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == 0 || pid == std::process::id() {
            return None;
        }
        let title_len = GetWindowTextW(hwnd, title.as_mut_ptr(), title.len() as i32);
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        let mut has_image = false;
        if !process.is_null() {
            has_image = QueryFullProcessImageNameW(
                process,
                PROCESS_NAME_WIN32,
                image.as_mut_ptr(),
                &mut image_len,
            ) != 0;
            CloseHandle(process);
        }
        (title_len, has_image)
    };
    let app = if has_image {
        let path = String::from_utf16_lossy(&image[..image_len as usize]);
        std::path::Path::new(&path)
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default()
    } else {
        String::new()
    };
    Some(Source {
        app,
        title: String::from_utf16_lossy(&title[..title_len.max(0) as usize]),
    })
}

pub fn set_exclusion(window: &winit::window::Window, enabled: bool) -> Result<()> {
    let RawWindowHandle::Win32(handle) = window.window_handle()?.as_raw() else {
        bail!("Expected an owned Windows top-level window");
    };
    let hwnd = handle.hwnd.get() as *mut std::ffi::c_void;
    let desired = if enabled {
        WDA_EXCLUDEFROMCAPTURE
    } else {
        WDA_NONE
    };
    let mut composition = 0;
    let mut actual = 0;
    // SAFETY: hwnd is borrowed from this live, current-process winit window;
    // output pointers refer to initialized stack values for the duration of each call.
    unsafe {
        if enabled && (DwmIsCompositionEnabled(&mut composition) < 0 || composition == 0) {
            bail!("Desktop composition is unavailable; recording exclusion cannot be enabled");
        }
        if SetWindowDisplayAffinity(hwnd, desired) == 0 {
            return Err(std::io::Error::last_os_error())
                .context("Windows could not change recording exclusion");
        }
        if GetWindowDisplayAffinity(hwnd, &mut actual) == 0 {
            return Err(std::io::Error::last_os_error())
                .context("Windows could not verify recording-exclusion state");
        }
    }
    if actual != desired {
        bail!("Windows did not apply the requested recording-exclusion state");
    }
    Ok(())
}

struct CaptureResources {
    screen: HDC,
    memory: HDC,
    bitmap: HBITMAP,
    previous: HGDIOBJ,
}
impl Drop for CaptureResources {
    fn drop(&mut self) {
        // SAFETY: each non-null resource is exclusively owned here and released once;
        // restore the original selected object before deleting our bitmap/DC.
        unsafe {
            if !self.previous.is_null() && !self.memory.is_null() {
                SelectObject(self.memory, self.previous);
            }
            if !self.bitmap.is_null() {
                DeleteObject(self.bitmap);
            }
            if !self.memory.is_null() {
                DeleteDC(self.memory);
            }
            if !self.screen.is_null() {
                ReleaseDC(null_mut(), self.screen);
            }
        }
    }
}

pub fn capture(display: &Display) -> Result<DynamicImage> {
    let width: i32 = display.width.try_into()?;
    let height: i32 = display.height.try_into()?;
    let mut resources = CaptureResources {
        screen: null_mut(),
        memory: null_mut(),
        bitmap: null_mut(),
        previous: null_mut(),
    };
    let mut pixels = vec![0_u8; display.width as usize * display.height as usize * 4];
    // SAFETY: dimensions are bounded before entry; Win32 resources are null-checked,
    // outputs are writable initialized buffers, and RAII covers every failure path.
    unsafe {
        if DwmFlush() < 0 {
            bail!("Cannot synchronize the compositor before capture");
        }
        resources.screen = GetDC(null_mut());
        if resources.screen.is_null() {
            return Err(std::io::Error::last_os_error()).context("Cannot access the desktop");
        }
        resources.memory = CreateCompatibleDC(resources.screen);
        resources.bitmap = CreateCompatibleBitmap(resources.screen, width, height);
        if resources.memory.is_null() || resources.bitmap.is_null() {
            return Err(std::io::Error::last_os_error())
                .context("Cannot allocate capture resources");
        }
        resources.previous = SelectObject(resources.memory, resources.bitmap);
        if resources.previous.is_null() || resources.previous as isize == -1 {
            resources.previous = null_mut();
            bail!("Cannot select capture bitmap");
        }
        if BitBlt(
            resources.memory,
            0,
            0,
            width,
            height,
            resources.screen,
            display.x,
            display.y,
            SRCCOPY | CAPTUREBLT,
        ) == 0
        {
            return Err(std::io::Error::last_os_error()).context("Desktop screenshot failed");
        }
        SelectObject(resources.memory, resources.previous);
        resources.previous = null_mut();
        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                ..std::mem::zeroed()
            },
            bmiColors: [std::mem::zeroed()],
        };
        if GetDIBits(
            resources.memory,
            resources.bitmap,
            0,
            display.height,
            pixels.as_mut_ptr().cast(),
            &mut info,
            DIB_RGB_COLORS,
        ) != height
        {
            return Err(std::io::Error::last_os_error()).context("Cannot read screenshot pixels");
        }
    }
    for pixel in pixels.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
        pixel[3] = 255;
    }
    let image = RgbaImage::from_raw(display.width, display.height, pixels)
        .context("Invalid screenshot pixel buffer")?;
    Ok(DynamicImage::ImageRgba8(image))
}
