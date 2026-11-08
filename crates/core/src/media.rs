use std::{
    fs::{self, File},
    io::{BufReader, Read, Write},
    path::Path,
};

use anyhow::{Context, Result, bail};
use image::{DynamicImage, ImageReader, Limits};
use sha2::{Digest, Sha256};

use crate::config::Paths;

pub const MAX_FILE_BYTES: u64 = 128 * 1024 * 1024;
pub const MAX_PIXELS: u64 = 64 * 1024 * 1024;

pub fn load_image(path: &Path) -> Result<DynamicImage> {
    let file =
        File::open(path).with_context(|| format!("Cannot open image: {}", path.display()))?;
    if file.metadata()?.len() > MAX_FILE_BYTES {
        bail!("Image file exceeds 128 MiB");
    }
    let mut reader = ImageReader::new(BufReader::new(file)).with_guessed_format()?;
    let mut limits = Limits::default();
    limits.max_alloc = Some(256 * 1024 * 1024);
    limits.max_image_width = Some(32768);
    limits.max_image_height = Some(32768);
    reader.limits(limits);
    let image = reader.decode().context("Cannot decode image")?;
    validate_dimensions(image.width(), image.height())?;
    Ok(image)
}

pub fn validate_dimensions(width: u32, height: u32) -> Result<()> {
    if width == 0 || height == 0 || u64::from(width) * u64::from(height) > MAX_PIXELS {
        bail!("Image dimensions are empty or exceed the 64-megapixel limit");
    }
    Ok(())
}

pub fn fingerprint(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    if file.metadata()?.len() > MAX_FILE_BYTES {
        bail!("Image file exceeds 128 MiB");
    }
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 65536];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hash.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

pub fn supported(path: &Path) -> bool {
    path.extension().and_then(|v| v.to_str()).is_some_and(|v| {
        ["png", "jpg", "jpeg", "webp"]
            .iter()
            .any(|e| v.eq_ignore_ascii_case(e))
    })
}

pub fn save_capture(image: &DynamicImage, destination: &Path) -> Result<std::path::PathBuf> {
    validate_dimensions(image.width(), image.height())?;
    fs::create_dir_all(destination).context("Cannot create capture directory")?;
    let mut temp = tempfile::Builder::new()
        .prefix(".oxide-")
        .suffix(".part")
        .tempfile_in(destination)?;
    image.write_to(&mut temp, image::ImageFormat::Png)?;
    temp.as_file().sync_all()?;
    let stem = chrono::Local::now()
        .format("Oxide %Y-%m-%d %H.%M.%S%.3f")
        .to_string();
    for suffix in 0..10000 {
        let path = destination.join(if suffix == 0 {
            format!("{stem}.png")
        } else {
            format!("{stem}-{suffix}.png")
        });
        match temp.persist_noclobber(&path) {
            Ok(_) => {
                #[cfg(unix)]
                {
                    File::open(destination)?.sync_all()?;
                }
                return Ok(path);
            }
            Err(e) if e.error.kind() == std::io::ErrorKind::AlreadyExists => {
                temp = e.file;
            }
            Err(e) => return Err(e.error).context("Cannot save screenshot original"),
        }
    }
    bail!("Cannot allocate a collision-free screenshot filename")
}

/// Paints opaque black over each normalized line box (plus a small margin).
/// Opaque fill, not blur: blurred text can sometimes be recovered.
pub fn redact(image: &DynamicImage, lines: &[crate::Line]) -> DynamicImage {
    let mut pixels = image.to_rgba8();
    let (width, height) = (pixels.width() as f32, pixels.height() as f32);
    for line in lines.iter().filter(|line| line.valid()) {
        let x0 = ((line.x * width).floor() - 2.0).max(0.0) as u32;
        let y0 = ((line.y * height).floor() - 2.0).max(0.0) as u32;
        let x1 = (((line.x + line.w) * width).ceil() + 2.0).min(width) as u32;
        let y1 = (((line.y + line.h) * height).ceil() + 2.0).min(height) as u32;
        for y in y0..y1 {
            for x in x0..x1 {
                pixels.put_pixel(x, y, image::Rgba([0, 0, 0, 255]));
            }
        }
    }
    DynamicImage::ImageRgba8(pixels)
}

/// Atomically replaces an original with new pixels in its existing format.
pub fn replace_image(path: &Path, image: &DynamicImage) -> Result<()> {
    let format = image::ImageFormat::from_path(path).context("Unsupported original format")?;
    let parent = path.parent().context("Original has no parent directory")?;
    let mut temp = tempfile::Builder::new()
        .prefix(".oxide-")
        .suffix(".part")
        .tempfile_in(parent)?;
    if format == image::ImageFormat::Jpeg {
        DynamicImage::ImageRgb8(image.to_rgb8()).write_to(&mut temp, format)?;
    } else {
        image.write_to(&mut temp, format)?;
    }
    temp.as_file().sync_all()?;
    temp.persist(path)
        .map_err(|e| e.error)
        .context("Cannot replace the original image")?;
    #[cfg(unix)]
    {
        File::open(parent)?.sync_all()?;
    }
    Ok(())
}

pub fn thumbnail(paths: &Paths, id: i64, image: &DynamicImage, source: &Path) -> Result<()> {
    paths.ensure_original_location(source)?;
    let thumb = image.thumbnail(480, 320).to_rgb8();
    let mut bytes = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 80).encode_image(&thumb)?;
    crate::config::atomic_write(&paths.thumbnail(id), &bytes)
}

pub fn export_note(store: &crate::store::Store, id: i64, destination: &Path) -> Result<()> {
    let note = store.note(id)?.context("Note no longer exists")?;
    fs::create_dir_all(destination)?;
    let mut text = format!(
        "# {}\n\n{}\n",
        note.title.replace(['\n', '\r'], " "),
        note.body
    );
    let shots = store.attachments(id)?;
    for shot in shots {
        let source = Path::new(&shot.path);
        if !source.is_file() {
            bail!("An attachment original is missing; export was not marked complete");
        }
        let extension = source.extension().and_then(|v| v.to_str()).unwrap_or("png");
        let name = format!("image-{}.{}", shot.id, extension);
        let assets = destination.join("images");
        fs::create_dir_all(&assets)?;
        let mut temp = tempfile::NamedTempFile::new_in(&assets)?;
        std::io::copy(&mut File::open(source)?, &mut temp)?;
        temp.flush()?;
        temp.as_file().sync_all()?;
        temp.persist_noclobber(assets.join(&name))
            .map_err(|e| e.error)
            .context("Export image already exists or cannot be saved")?;
        text.push_str(&format!("\n![Screenshot {}](images/{name})\n", shot.id));
    }
    let mut temp = tempfile::NamedTempFile::new_in(destination)?;
    temp.write_all(text.as_bytes())?;
    temp.as_file().sync_all()?;
    temp.persist_noclobber(destination.join(format!("note-{id}.md")))
        .map_err(|e| e.error)
        .context("Export note already exists or cannot be saved")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redaction_blacks_out_only_the_line_box_and_survives_reload() -> Result<()> {
        let root = tempfile::tempdir()?;
        let path = root.path().join("shot.png");
        image::RgbImage::from_pixel(100, 100, image::Rgb([255, 255, 255])).save(&path)?;
        let line = crate::Line {
            text: "secret".into(),
            x: 0.2,
            y: 0.2,
            w: 0.3,
            h: 0.1,
            confidence: None,
        };
        let redacted = redact(&load_image(&path)?, &[line]);
        replace_image(&path, &redacted)?;
        let reloaded = load_image(&path)?.to_rgba8();
        assert_eq!(reloaded.get_pixel(30, 25).0, [0, 0, 0, 255]);
        assert_eq!(reloaded.get_pixel(90, 90).0, [255, 255, 255, 255]);
        Ok(())
    }
}
