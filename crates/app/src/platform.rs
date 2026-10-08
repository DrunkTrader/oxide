use std::{path::Path, sync::Arc};

#[cfg(target_os = "linux")]
use anyhow::Context;
use anyhow::{Result, bail};
use image::DynamicImage;
use winit::window::Window;

#[cfg(target_os = "linux")]
#[path = "platform_linux.rs"]
mod linux;
#[cfg(windows)]
#[path = "platform_windows.rs"]
mod windows;

#[derive(Clone, Debug)]
pub struct Display {
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

pub fn displays(window: &Window) -> Vec<Display> {
    window
        .available_monitors()
        .enumerate()
        .map(|(index, monitor)| {
            let position = monitor.position();
            let size = monitor.size();
            Display {
                name: monitor
                    .name()
                    .unwrap_or_else(|| format!("Display {}", index + 1)),
                x: position.x,
                y: position.y,
                width: size.width,
                height: size.height,
            }
        })
        .collect()
}

pub fn exclusion_supported() -> bool {
    cfg!(windows)
}

pub fn set_exclusion(window: &Window, enabled: bool) -> Result<()> {
    #[cfg(windows)]
    {
        windows::set_exclusion(window, enabled)
    }
    #[cfg(not(windows))]
    {
        let _ = window;
        if enabled {
            bail!(
                "Recording exclusion is unavailable on this platform; notes and screenshots work with it Off"
            );
        }
        Ok(())
    }
}

pub fn hide_for_capture(window: &Arc<Window>) -> Result<()> {
    window.set_visible(false);
    #[cfg(target_os = "linux")]
    {
        linux::unmap(window)?;
    }
    Ok(())
}

pub fn capture(display: &Display) -> Result<DynamicImage> {
    oxide_core::media::validate_dimensions(display.width, display.height)?;
    #[cfg(windows)]
    {
        windows::capture(display)
    }
    #[cfg(target_os = "linux")]
    {
        linux::capture(display)
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        let _ = display;
        bail!("Native screen capture is not available in this platform build")
    }
}

pub fn startup(enabled: bool, data: &Path) -> Result<()> {
    let exe = std::env::current_exe()?;
    #[cfg(windows)]
    {
        let key = "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run";
        let output = if enabled {
            std::process::Command::new("reg.exe")
                .args([
                    "add",
                    key,
                    "/v",
                    "Oxide",
                    "/t",
                    "REG_SZ",
                    "/d",
                    &format!(
                        "\"{}\" --background --data-dir \"{}\"",
                        exe.display(),
                        data.display()
                    ),
                    "/f",
                ])
                .output()?
        } else {
            std::process::Command::new("reg.exe")
                .args(["delete", key, "/v", "Oxide", "/f"])
                .output()?
        };
        if !output.status.success() && enabled {
            bail!("Windows login startup registration failed");
        }
        Ok(())
    }
    #[cfg(target_os = "linux")]
    {
        let home =
            std::env::var_os("HOME").context("Cannot locate home for startup registration")?;
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from(home).join(".config"));
        let path = base.join("autostart/oxide.desktop");
        if enabled {
            let quote = |path: &Path| -> Result<String> {
                let text = path.to_str().context("Startup path must be Unicode")?;
                if text.chars().any(char::is_control) {
                    bail!("Startup paths cannot contain control characters");
                }
                Ok(format!(
                    "\"{}\"",
                    text.replace('\\', "\\\\")
                        .replace('"', "\\\"")
                        .replace('$', "\\$")
                        .replace('`', "\\`")
                ))
            };
            let text = format!(
                "[Desktop Entry]\nType=Application\nName=Oxide\nExec={} --background --data-dir {}\nTerminal=false\n",
                quote(&exe)?,
                quote(data)?
            );
            oxide_core::config::atomic_write(&path, text.as_bytes())?;
        } else if path.exists() {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        let _ = (enabled, data, exe);
        bail!("Login startup is unavailable in this platform build")
    }
}
