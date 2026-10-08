//! CPU OCR runs in the reader, never in the window process.

use std::{io::Read, path::Path, time::Duration};

use anyhow::{Context, Result, bail};
use image::DynamicImage;
use ocrs::{ImageSource, OcrEngine, OcrEngineParams, TextItem};
use oxide_core::{Line, config::atomic_write, media};
use sha2::{Digest, Sha256};

pub const VERSION: &str = "ocrs-0.13.1";

const MODELS: [(&str, &str); 2] = [
    (
        "text-detection.onnx",
        "a917b23dbd9524b465df7e922641b3ff2981623df4ded5a0234004ef2fee7cfe",
    ),
    (
        "text-recognition.onnx",
        "86c145c2edb96c8caed5b1ebb8f44d706408922211451c309c625157dd6061c5",
    ),
];

pub struct Reader {
    engine: OcrEngine,
    pool: rayon::ThreadPool,
}

impl Reader {
    pub fn prepare(directory: &Path, threads: usize, offline: bool) -> Result<Self> {
        for (name, digest) in MODELS {
            let path = directory.join(name);
            if path.exists() {
                if media::fingerprint(&path)? != digest {
                    bail!(
                        "OCR model checksum mismatch: {name}. Replace it with the pinned model; the file was preserved"
                    );
                }
                continue;
            }
            if offline {
                bail!("OCR model {name} is missing; provision models or disable offline mode");
            }
            let agent: ureq::Agent = ureq::Agent::config_builder()
                .timeout_global(Some(Duration::from_secs(120)))
                .build()
                .into();
            let mut response = agent
                .get(format!(
                    "https://ocrs-models.s3-accelerate.amazonaws.com/{name}"
                ))
                .call()
                .with_context(|| format!("Cannot download OCR model {name}"))?;
            let mut bytes = Vec::new();
            response
                .body_mut()
                .as_reader()
                .take(media::MAX_FILE_BYTES + 1)
                .read_to_end(&mut bytes)?;
            if bytes.len() as u64 > media::MAX_FILE_BYTES {
                bail!("OCR model download exceeded 128 MiB");
            }
            if format!("{:x}", Sha256::digest(&bytes)) != digest {
                bail!("Downloaded OCR model checksum mismatch: {name}");
            }
            atomic_write(&path, &bytes)?;
        }
        let engine = OcrEngine::new(OcrEngineParams {
            detection_model: Some(
                rten::Model::load_file(directory.join(MODELS[0].0))
                    .context("Cannot load text detector")?,
            ),
            recognition_model: Some(
                rten::Model::load_file(directory.join(MODELS[1].0))
                    .context("Cannot load text recognizer")?,
            ),
            ..Default::default()
        })
        .context("Cannot initialize local OCR")?;
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads.clamp(1, 16))
            .build()?;
        Ok(Self { engine, pool })
    }

    pub fn read(&self, image: &DynamicImage) -> Result<Vec<Line>> {
        media::validate_dimensions(image.width(), image.height())?;
        // Bound inference tensors independently of the original/preview size.
        let input_image = if image.width().max(image.height()) > 2400 {
            image.thumbnail(2400, 2400)
        } else {
            image.clone()
        }
        .into_rgb8();
        self.pool.install(|| {
            let source = ImageSource::from_bytes(input_image.as_raw(), input_image.dimensions())?;
            let input = self.engine.prepare_input(source)?;
            let words = self.engine.detect_words(&input)?;
            let regions = self.engine.find_text_lines(&input, &words);
            let recognized = self.engine.recognize_text(&input, &regions)?;
            let width = input_image.width() as f32;
            let height = input_image.height() as f32;
            let mut lines = Vec::new();
            for line in recognized.into_iter().flatten() {
                let text = line.to_string();
                if text.trim().is_empty() {
                    continue;
                }
                let rect = line.bounding_rect();
                let x = (rect.left() as f32 / width).clamp(0.0, 1.0);
                let y = (rect.top() as f32 / height).clamp(0.0, 1.0);
                let right = (rect.right() as f32 / width).clamp(x, 1.0);
                let bottom = (rect.bottom() as f32 / height).clamp(y, 1.0);
                let result = Line {
                    text,
                    x,
                    y,
                    w: right - x,
                    h: bottom - y,
                    confidence: None,
                };
                if result.valid() {
                    lines.push(result);
                }
            }
            Ok(lines)
        })
    }
}
