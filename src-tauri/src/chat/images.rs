//! Conversation images: bounded decoding, local storage, and owned references.

use anyhow::{anyhow, bail, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use image::{ImageDecoder, ImageFormat, ImageReader};
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use std::path::PathBuf;

use super::conversations::Conversations;
use crate::{engine::vision::VisionLimits, paths::Paths};

pub const MAX_IMAGE_BYTES: usize = 20 * 1024 * 1024;
const MAX_PIXELS: u64 = 40_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatImage {
    pub id: String,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub bytes: u64,
}

fn directory(paths: &Paths, thread: &str) -> Result<PathBuf> {
    crate::ids::require_safe_id(thread)?;
    if Conversations::get(paths, thread).is_none() {
        bail!("thread not found");
    }
    Ok(paths.conversations_dir().join(thread).join("images"))
}

fn file(paths: &Paths, thread: &str, id: &str, suffix: &str) -> Result<PathBuf> {
    crate::ids::require_safe_id(id)?;
    Ok(directory(paths, thread)?.join(format!("{id}.{suffix}")))
}

pub fn decode_base64(encoded: &str) -> Result<Vec<u8>> {
    if encoded.len() > MAX_IMAGE_BYTES * 4 / 3 + 4 {
        bail!("image-too-large");
    }
    STANDARD.decode(encoded).context("image-invalid")
}

/// Normalize orientation and strip metadata before an image enters the conversation.
pub fn store(
    paths: &Paths,
    thread: &str,
    name: &str,
    bytes: &[u8],
    limits: VisionLimits,
) -> Result<ChatImage> {
    if bytes.len() > MAX_IMAGE_BYTES {
        bail!("image-too-large");
    }
    let mut reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    if !matches!(reader.format(), Some(ImageFormat::Png | ImageFormat::Jpeg)) {
        bail!("image-format");
    }
    let mut decode_limits = image::Limits::default();
    decode_limits.max_image_width = Some(16384);
    decode_limits.max_image_height = Some(16384);
    decode_limits.max_alloc = Some(192 * 1024 * 1024);
    reader.limits(decode_limits);
    let mut decoder = reader
        .into_decoder()
        .map_err(|_| anyhow!("image-invalid"))?;
    let (width, height) = decoder.dimensions();
    if u64::from(width) * u64::from(height) > MAX_PIXELS {
        bail!("image-too-large");
    }
    let orientation = decoder
        .orientation()
        .unwrap_or(image::metadata::Orientation::NoTransforms);
    let mut image =
        image::DynamicImage::from_decoder(decoder).map_err(|_| anyhow!("image-invalid"))?;
    image.apply_orientation(orientation);
    let image = fit_image(image, limits.max_edge);
    let mut encoded = Cursor::new(Vec::new());
    image.write_to(&mut encoded, ImageFormat::Png)?;
    let mut thumbnail = Cursor::new(Vec::new());
    image
        .thumbnail(320, 240)
        .write_to(&mut thumbnail, ImageFormat::Png)?;
    let dir = directory(paths, thread)?;
    let lock = crate::paths::metadata_lock(&dir);
    let _guard = crate::core::mutex_lock(&lock);
    std::fs::create_dir_all(&dir)?;
    let used: u64 = std::fs::read_dir(&dir)?
        .filter_map(|entry| entry.ok()?.metadata().ok())
        .map(|meta| meta.len())
        .sum();
    if used.saturating_add(encoded.get_ref().len() as u64) > 256 * 1024 * 1024 {
        bail!("image-storage-full");
    }
    let id = format!("img_{}", uuid::Uuid::new_v4().simple());
    let name: String = name
        .chars()
        .filter(|c| !c.is_control() && !matches!(c, '/' | '\\'))
        .take(120)
        .collect();
    let meta = ChatImage {
        id: id.clone(),
        name,
        width: image.width(),
        height: image.height(),
        bytes: encoded.get_ref().len() as u64,
    };
    std::fs::write(dir.join(format!("{id}.png")), encoded.into_inner())?;
    std::fs::write(dir.join(format!("{id}.thumb.png")), thumbnail.into_inner())?;
    std::fs::write(dir.join(format!("{id}.json")), serde_json::to_vec(&meta)?)?;
    Ok(meta)
}

pub fn resolve(
    paths: &Paths,
    thread: &str,
    ids: &[String],
    limits: VisionLimits,
) -> Result<Vec<ChatImage>> {
    if ids.len() > limits.max_images {
        bail!("image-count");
    }
    let mut seen = std::collections::HashSet::new();
    ids.iter()
        .map(|id| {
            if !seen.insert(id) {
                bail!("image-invalid");
            }
            let meta: ChatImage = serde_json::from_slice(
                &std::fs::read(file(paths, thread, id, "json")?).context("image-missing")?,
            )?;
            if meta.id != *id || meta.width > limits.max_edge || meta.height > limits.max_edge {
                bail!("image-limits-changed");
            }
            Ok(meta)
        })
        .collect()
}

pub fn data_url(paths: &Paths, thread: &str, id: &str, thumbnail: bool) -> Result<String> {
    let path = file(
        paths,
        thread,
        id,
        if thumbnail { "thumb.png" } else { "png" },
    )?;
    if std::fs::metadata(&path).context("image-missing")?.len() > MAX_IMAGE_BYTES as u64 {
        bail!("image-too-large");
    }
    Ok(format!(
        "data:image/png;base64,{}",
        STANDARD.encode(std::fs::read(path).context("image-missing")?)
    ))
}

fn fit_image(image: image::DynamicImage, edge: u32) -> image::DynamicImage {
    if image.width() <= edge && image.height() <= edge {
        image
    } else {
        image.resize(edge, edge, image::imageops::FilterType::Lanczos3)
    }
}

/// Older conversations may have been created with a larger image budget.
pub fn inference_url(paths: &Paths, thread: &str, image: &ChatImage, edge: u32) -> Result<String> {
    if image.width <= edge && image.height <= edge {
        return data_url(paths, thread, &image.id, false);
    }
    let path = file(paths, thread, &image.id, "png")?;
    if std::fs::metadata(&path).context("image-missing")?.len() > MAX_IMAGE_BYTES as u64 {
        bail!("image-too-large");
    }
    let mut reader = ImageReader::open(path)?.with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(1536);
    limits.max_image_height = Some(1536);
    limits.max_alloc = Some(32 * 1024 * 1024);
    reader.limits(limits);
    let mut encoded = Cursor::new(Vec::new());
    fit_image(reader.decode()?, edge).write_to(&mut encoded, ImageFormat::Png)?;
    Ok(format!(
        "data:image/png;base64,{}",
        STANDARD.encode(encoded.into_inner())
    ))
}

pub fn remove_draft(paths: &Paths, thread: &str, id: &str) -> Result<()> {
    let dir = directory(paths, thread)?;
    crate::ids::require_safe_id(id)?;
    if Conversations::messages(paths, thread)
        .iter()
        .any(|m| m.images.iter().any(|image| image.id == id))
    {
        return Ok(());
    }
    for suffix in ["png", "thumb.png", "json"] {
        match std::fs::remove_file(dir.join(format!("{id}.{suffix}"))) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

/// Keep Markdown exports portable without pointing back into private app data.
pub fn export(paths: &Paths, thread: &str, ids: &[String], parent: &std::path::Path) -> Result<()> {
    if ids.is_empty() {
        return Ok(());
    }
    let output = parent.join("rebost-images");
    std::fs::create_dir_all(&output)?;
    for id in ids {
        let source = file(paths, thread, id, "png")?;
        std::fs::copy(source, output.join(format!("{id}.png"))).context("image-missing")?;
    }
    Ok(())
}

/// Drafts live in the current UI session. Reclaim abandoned files before the window is usable.
pub fn cleanup_drafts(paths: &Paths) -> Result<()> {
    for thread in Conversations::list(paths) {
        let dir = directory(paths, &thread.id)?;
        if !dir.exists() {
            continue;
        }
        let referenced: std::collections::HashSet<_> = Conversations::messages(paths, &thread.id)
            .into_iter()
            .flat_map(|message| message.images.into_iter().map(|image| image.id))
            .collect();
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let Some(id) = name.split('.').next().filter(|id| id.starts_with("img_")) else {
                continue;
            };
            if !referenced.contains(id) && entry.file_type()?.is_file() {
                std::fs::remove_file(entry.path())?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn image_storage_is_bounded_and_scoped_to_its_conversation() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        let a = Conversations::create(&paths, None).unwrap();
        let b = Conversations::create(&paths, None).unwrap();
        let limits = VisionLimits {
            max_images: 1,
            max_edge: 768,
            tokens_per_image: 1024,
        };
        let source = image::DynamicImage::new_rgb8(1600, 800);
        let mut png = Cursor::new(Vec::new());
        source.write_to(&mut png, ImageFormat::Png).unwrap();
        let saved = store(&paths, &a.id, "screen.png", png.get_ref(), limits).unwrap();
        assert_eq!((saved.width, saved.height), (768, 384));
        assert!(data_url(&paths, &a.id, &saved.id, true)
            .unwrap()
            .starts_with("data:image/png;base64,"));
        assert!(resolve(&paths, &b.id, std::slice::from_ref(&saved.id), limits).is_err());
        assert!(data_url(&paths, &a.id, "../escape", false).is_err());
        assert!(resolve(&paths, &a.id, &[saved.id.clone(), saved.id.clone()], limits).is_err());
        assert!(store(&paths, &a.id, "fake.png", b"not an image", limits).is_err());
        remove_draft(&paths, &a.id, &saved.id).unwrap();
        assert!(data_url(&paths, &a.id, &saved.id, false).is_err());
    }

    #[test]
    fn small_images_stay_small_and_history_obeys_a_smaller_budget() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        let thread = Conversations::create(&paths, None).unwrap();
        let limits = VisionLimits {
            max_images: 2,
            max_edge: 1024,
            tokens_per_image: 1024,
        };
        let mut png = Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(400, 200)
            .write_to(&mut png, ImageFormat::Png)
            .unwrap();
        let saved = store(&paths, &thread.id, "small.png", png.get_ref(), limits).unwrap();
        assert_eq!((saved.width, saved.height), (400, 200));
        let url = inference_url(&paths, &thread.id, &saved, 128).unwrap();
        let bytes = STANDARD.decode(url.split_once(',').unwrap().1).unwrap();
        let fitted = image::load_from_memory(&bytes).unwrap();
        assert_eq!((fitted.width(), fitted.height()), (128, 64));
        let message: super::super::conversations::StoredMessage =
            serde_json::from_value(serde_json::json!({
                "id":"message", "role":"user", "text":"look", "ts":"today", "images":[saved]
            }))
            .unwrap();
        Conversations::append(&paths, &thread.id, &message).unwrap();
        let abandoned = store(&paths, &thread.id, "draft.png", png.get_ref(), limits).unwrap();
        cleanup_drafts(&paths).unwrap();
        assert!(data_url(&paths, &thread.id, &abandoned.id, false).is_err());
        remove_draft(&paths, &thread.id, &saved.id).unwrap();
        assert!(data_url(&paths, &thread.id, &saved.id, false).is_ok());
        let output = tempfile::tempdir().unwrap();
        export(
            &paths,
            &thread.id,
            std::slice::from_ref(&saved.id),
            output.path(),
        )
        .unwrap();
        let markdown = super::super::conversations::thread_markdown("Images", &[message]);
        assert!(markdown.contains(&format!("rebost-images/{}.png", saved.id)));
        assert!(output
            .path()
            .join("rebost-images")
            .join(format!("{}.png", saved.id))
            .is_file());
    }
}
