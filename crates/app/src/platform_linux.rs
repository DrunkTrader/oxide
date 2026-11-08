use anyhow::{Context, Result, bail};
use image::{DynamicImage, RgbaImage};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use x11rb::{
    connection::Connection,
    protocol::xproto::{AtomEnum, ConnectionExt, ImageFormat, ImageOrder},
};

use super::{Display, Source};

pub fn foreground_source() -> Result<Option<Source>> {
    let (connection, index) = x11rb::connect(None)?;
    let root = connection.setup().roots[index].root;
    let atom = |name: &str| -> Result<u32> {
        Ok(connection
            .intern_atom(false, name.as_bytes())?
            .reply()?
            .atom)
    };
    let text = |window: u32, property: u32, kind: u32| -> Result<Vec<u8>> {
        Ok(connection
            .get_property(false, window, property, kind, 0, 1024)?
            .reply()?
            .value)
    };
    let active = connection
        .get_property(
            false,
            root,
            atom("_NET_ACTIVE_WINDOW")?,
            AtomEnum::WINDOW,
            0,
            1,
        )?
        .reply()?;
    let Some(window) = active
        .value32()
        .and_then(|mut v| v.next())
        .filter(|w| *w != 0)
    else {
        return Ok(None);
    };
    let pid = connection
        .get_property(
            false,
            window,
            atom("_NET_WM_PID")?,
            AtomEnum::CARDINAL,
            0,
            1,
        )?
        .reply()?
        .value32()
        .and_then(|mut v| v.next());
    if pid == Some(std::process::id()) {
        return Ok(None);
    }
    let mut title = text(window, atom("_NET_WM_NAME")?, atom("UTF8_STRING")?)?;
    if title.is_empty() {
        title = text(window, AtomEnum::WM_NAME.into(), AtomEnum::STRING.into())?;
    }
    // WM_CLASS is "instance\0Class\0"; the class names the application.
    let class = text(window, AtomEnum::WM_CLASS.into(), AtomEnum::STRING.into())?;
    let app = class
        .split(|byte| *byte == 0)
        .rfind(|part| !part.is_empty())
        .map(|part| String::from_utf8_lossy(part).into_owned())
        .unwrap_or_default();
    Ok(Some(Source {
        app,
        title: String::from_utf8_lossy(&title).into_owned(),
    }))
}

pub fn unmap(window: &winit::window::Window) -> Result<()> {
    let id = match window.window_handle()?.as_raw() {
        RawWindowHandle::Xlib(handle) => u32::try_from(handle.window)?,
        RawWindowHandle::Xcb(handle) => handle.window.get(),
        _ => bail!("Native screenshot capture currently requires an X11 window/session"),
    };
    let (connection, _) = x11rb::connect(None)?;
    connection.unmap_window(id)?.check()?;
    connection.get_input_focus()?.reply()?;
    Ok(())
}

pub fn capture(display: &Display) -> Result<DynamicImage> {
    let (connection, index) =
        x11rb::connect(None).context("Screen capture requires an available X11 session")?;
    let setup = connection.setup();
    let screen = &setup.roots[index];
    if display.x < 0
        || display.y < 0
        || display.x as u32 + display.width > u32::from(screen.width_in_pixels)
        || display.y as u32 + display.height > u32::from(screen.height_in_pixels)
    {
        bail!("Selected display is outside the X11 root image");
    }
    let image = connection
        .get_image(
            ImageFormat::Z_PIXMAP,
            screen.root,
            display.x.try_into()?,
            display.y.try_into()?,
            display.width.try_into()?,
            display.height.try_into()?,
            u32::MAX,
        )?
        .reply()?;
    let format = setup
        .pixmap_formats
        .iter()
        .find(|format| format.depth == image.depth)
        .context("Unknown X11 image format")?;
    let visual = screen
        .allowed_depths
        .iter()
        .flat_map(|depth| &depth.visuals)
        .find(|visual| visual.visual_id == screen.root_visual)
        .context("Unknown X11 root visual")?;
    let bytes_per_pixel = usize::from(format.bits_per_pixel / 8);
    if !(3..=4).contains(&bytes_per_pixel) {
        bail!("X11 capture requires a 24/32-bit desktop");
    }
    let stride = (display.width as usize * usize::from(format.bits_per_pixel))
        .div_ceil(usize::from(format.scanline_pad))
        * usize::from(format.scanline_pad)
        / 8;
    if image.data.len() < stride * display.height as usize {
        bail!("X11 returned a truncated screen image");
    }
    let mut rgba = RgbaImage::new(display.width, display.height);
    for y in 0..display.height {
        for x in 0..display.width {
            let offset = y as usize * stride + x as usize * bytes_per_pixel;
            let bytes = &image.data[offset..offset + bytes_per_pixel];
            let mut pixel = 0_u32;
            if setup.image_byte_order == ImageOrder::LSB_FIRST {
                for (index, byte) in bytes.iter().enumerate() {
                    pixel |= u32::from(*byte) << (index * 8);
                }
            } else {
                for byte in bytes {
                    pixel = (pixel << 8) | u32::from(*byte);
                }
            }
            let channel = |mask: u32| {
                if mask == 0 {
                    0
                } else {
                    (((pixel & mask) >> mask.trailing_zeros()) * 255
                        / (mask >> mask.trailing_zeros())) as u8
                }
            };
            rgba.put_pixel(
                x,
                y,
                image::Rgba([
                    channel(visual.red_mask),
                    channel(visual.green_mask),
                    channel(visual.blue_mask),
                    255,
                ]),
            );
        }
    }
    Ok(DynamicImage::ImageRgba8(rgba))
}
