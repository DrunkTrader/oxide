//! Real OCR/storage/export check with a generated, nonprivate screenshot.
use std::{path::PathBuf, time::Instant};

use ab_glyph::{Font, FontRef, PxScale, ScaleFont, point};
use anyhow::{Context, Result, ensure};
use image::{DynamicImage, Rgb, RgbImage};
use oxide_core::{
    config::{Config, Paths},
    media,
    query::Query,
    store::Store,
};

fn main() -> Result<()> {
    let root = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .context("Pass a disposable workspace directory")?;
    let paths = Paths::discover(Some(root))?;
    let config = Config {
        capture_directory: paths.data.join("captures"),
        ..Default::default()
    };
    config.save(&paths.config)?;
    let image = fixture()?;
    let original = media::save_capture(&DynamicImage::ImageRgb8(image), &config.capture_directory)?;
    let mut store = Store::open(&paths.database())?;
    let mut note = store.new_note()?;
    note.title = "Oxide smoke note".into();
    note.body = "Saved decisions with screenshot evidence: 100% _ literal.".into();
    let note = store.save_note(&note)?;
    let shot = store.register_image(&original, true, Some(oxide_core::now()))?;
    ensure!(
        shot.time_source == "captured",
        "Capture timestamp provenance is missing"
    );
    store.attach(note.id, shot.id)?;
    let reader = oxide_ocr::Reader::prepare(
        &paths.models(),
        2,
        std::env::var_os("OXIDE_OFFLINE").is_some(),
    )?;
    let started = Instant::now();
    let lines = reader.read(&media::load_image(&original)?)?;
    let timing = started.elapsed();
    let text = lines
        .iter()
        .map(|line| line.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    ensure!(
        text.contains("Oxide"),
        "Real OCR did not recognize the fixture: {text}"
    );
    ensure!(lines.iter().all(|line| line.valid()), "Invalid geometry");
    store.set_ocr(&shot, &lines, oxide_ocr::VERSION)?;
    media::thumbnail(&paths, shot.id, &media::load_image(&original)?, &original)?;
    ensure!(
        store
            .screenshots(&Query::parse("Oxide local")?, 50, 0)?
            .iter()
            .any(|item| item.id == shot.id),
        "Actual indexed search did not return the original"
    );
    ensure!(
        store
            .notes(&Query::parse("100% _")?, false, 50, 0)?
            .iter()
            .any(|item| item.id == note.id),
        "Literal note search failed"
    );
    let export = paths.data.join(format!("export-{}", note.id));
    media::export_note(&store, note.id, &export)?;
    ensure!(
        export.join(format!("note-{}.md", note.id)).is_file(),
        "Markdown export is missing"
    );
    store.clear_derived()?;
    ensure!(
        store.attachments(note.id)?[0].id == shot.id,
        "Cache reset broke attachment identity"
    );
    drop(store);
    let store = Store::open(&paths.database())?;
    ensure!(
        store.note(note.id)?.context("Saved note is missing")?.body == note.body,
        "Acknowledged note did not survive restart"
    );
    println!(
        "PASS: real OCR → indexed screenshot search → attachment → portable note export → cache reset → restart"
    );
    println!(
        "Fixture: 1080×640; {} recognized lines; OCR {:?}; workspace {}",
        lines.len(),
        timing,
        paths.data.display()
    );
    Ok(())
}

fn fixture() -> Result<RgbImage> {
    let mut image = RgbImage::from_pixel(1080, 640, Rgb([248, 248, 244]));
    let font = FontRef::try_from_slice(epaint_default_fonts::UBUNTU_LIGHT)?;
    let scale = PxScale::from(42.0);
    let scaled = font.as_scaled(scale);
    for (row, text) in [
        "Oxide local notes",
        "Screenshot search keeps the original.",
        "Every saved decision stays on your device.",
    ]
    .iter()
    .enumerate()
    {
        let mut x = 50.0;
        for ch in text.chars() {
            let glyph = scaled.scaled_glyph(ch);
            let advance = scaled.h_advance(glyph.id);
            let glyph = glyph
                .id
                .with_scale_and_position(scale, point(x, 100.0 + row as f32 * 100.0));
            if let Some(outline) = font.outline_glyph(glyph) {
                let bounds = outline.px_bounds();
                outline.draw(|gx, gy, coverage| {
                    let px = bounds.min.x as i32 + gx as i32;
                    let py = bounds.min.y as i32 + gy as i32;
                    if px >= 0 && py >= 0 && px < 1080 && py < 640 {
                        let shade = (248.0 * (1.0 - coverage) + 28.0 * coverage) as u8;
                        image.put_pixel(px as u32, py as u32, Rgb([shade, shade, shade]));
                    }
                });
            }
            x += advance;
        }
    }
    Ok(image)
}
