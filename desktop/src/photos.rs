//! Profile photos: picked from a file, turned the right way up, cropped in the app to 4:5,
//! and kept as a JPEG in Your Memi's own folder. Nothing leaves the computer except inside an
//! encrypted sync backup, if you turn sync on.

use anyhow::{Context, Result};
use image::{imageops::FilterType, DynamicImage, ImageDecoder, ImageReader};
use std::io::Cursor;
use std::path::{Path, PathBuf};

/// Slightly portrait, the same as on Android.
pub const ASPECT: f32 = 4.0 / 5.0;
const LOAD_EDGE: u32 = 2048;
const SAVE_WIDTH: u32 = 1080;
/// Refuse anything absurd before decoding it (decompression bombs).
const MAX_FILE_BYTES: u64 = 60 * 1024 * 1024;

/// Decodes a photo, applies its EXIF rotation and shrinks it to at most 2048 on the long edge.
pub fn load_for_cropping(path: &Path) -> Result<DynamicImage> {
    let len = std::fs::metadata(path)?.len();
    anyhow::ensure!(len <= MAX_FILE_BYTES, "That photo is too large.");
    let bytes = std::fs::read(path)?;
    let mut reader = ImageReader::new(Cursor::new(&bytes)).with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(20_000);
    limits.max_image_height = Some(20_000);
    limits.max_alloc = Some(512 * 1024 * 1024);
    reader.limits(limits);
    let mut decoder = reader.into_decoder().context("Unsupported image")?;
    let orientation = decoder.orientation().ok();
    let mut img = DynamicImage::from_decoder(decoder).context("Couldn't read that photo")?;
    if let Some(o) = orientation { img.apply_orientation(o); }
    if img.width().max(img.height()) > LOAD_EDGE {
        img = img.resize(LOAD_EDGE, LOAD_EDGE, FilterType::Triangle);
    }
    Ok(img)
}

/// Saves the cropped area (x, y, w, h in the image's pixels) as the profile photo.
pub fn save_crop(img: &DynamicImage, x: u32, y: u32, w: u32, h: u32, dir: &Path) -> Result<PathBuf> {
    let x = x.min(img.width().saturating_sub(1));
    let y = y.min(img.height().saturating_sub(1));
    let w = w.clamp(1, img.width() - x);
    let h = h.clamp(1, img.height() - y);
    let mut out = img.crop_imm(x, y, w, h);
    if out.width() > SAVE_WIDTH {
        let nh = (out.height() as f32 * SAVE_WIDTH as f32 / out.width() as f32).round() as u32;
        out = out.resize_exact(SAVE_WIDTH, nh.max(1), FilterType::Lanczos3);
    }
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("{}.jpg", uuid::Uuid::new_v4()));
    // Re-encoding drops all metadata (location, camera) from the original file.
    let rgb = out.to_rgb8();
    let mut file = std::fs::File::create(&path)?;
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut file, 90).encode_image(&rgb)?;
    Ok(path)
}

/// Saves a photo that arrived in a sync backup (already cropped) under a new name.
pub fn save_bytes(bytes: &[u8], dir: &Path) -> Result<PathBuf> {
    // Decode first so only real images are written to disk.
    let img = image::load_from_memory(bytes).context("Not an image")?;
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("{}.jpg", uuid::Uuid::new_v4()));
    let mut file = std::fs::File::create(&path)?;
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut file, 90).encode_image(&img.to_rgb8())?;
    Ok(path)
}

/// Deletes a photo, but only if it lives in Your Memi's own photo folder.
pub fn delete(path: Option<&str>, dir: &Path) {
    let Some(p) = path else { return };
    let p = Path::new(p);
    if p.parent().map(|d| d == dir).unwrap_or(false) {
        let _ = std::fs::remove_file(p);
    }
}
