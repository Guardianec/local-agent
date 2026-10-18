//! Native ingestion owns decoding, limits, encrypted storage and opaque IDs.
use crate::{domain::*, store::Store};
use base64::{engine::general_purpose::STANDARD, Engine};
use image::{ImageDecoder, ImageFormat, ImageReader};
use std::io::Cursor;

pub const MAX_IMAGE_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_IMAGES: usize = 4;
pub fn ingest(store: &Store, name: &str, bytes: &[u8], source: &str) -> Result<ImageAttachment> {
    if bytes.is_empty() || bytes.len() > MAX_IMAGE_BYTES {
        return Err(AppError::new(
            "IMAGE_SIZE",
            "Choose an image up to 8 MiB. A task can contain up to four images.",
        ));
    }
    let bad = || {
        AppError::new(
            "IMAGE_FORMAT",
            "This image could not be decoded. Choose a valid PNG, JPEG or WebP image.",
        )
    };
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| bad())?;
    if !matches!(
        reader.format(),
        Some(ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::WebP)
    ) {
        return Err(bad());
    }
    let (width, height) = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| bad())?
        .into_dimensions()
        .map_err(|_| bad())?;
    if width == 0
        || height == 0
        || width > 16000
        || height > 16000
        || width as u64 * height as u64 > 40_000_000
    {
        return Err(AppError::new(
            "IMAGE_DIMENSIONS",
            "The image is too large to decode. Export a smaller image (at most 40 megapixels).",
        ));
    }
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(192 * 1024 * 1024);
    limits.max_image_width = Some(16000);
    limits.max_image_height = Some(16000);
    reader.limits(limits);
    let mut decoder = reader.into_decoder().map_err(|_| bad())?;
    let orientation = decoder
        .orientation()
        .unwrap_or(image::metadata::Orientation::NoTransforms);
    let mut decoded = image::DynamicImage::from_decoder(decoder).map_err(|_| bad())?;
    decoded.apply_orientation(orientation);
    let resized = width > 2048 || height > 2048;
    let normalized = if resized {
        decoded.resize(2048, 2048, image::imageops::FilterType::Lanczos3)
    } else {
        decoded
    };
    let mut png = Cursor::new(Vec::new());
    normalized
        .write_to(&mut png, ImageFormat::Png)
        .map_err(|_| bad())?;
    if png.get_ref().len() > 20 * 1024 * 1024 {
        return Err(AppError::new(
            "IMAGE_SIZE",
            "The decoded image is too large. Export a smaller image.",
        ));
    }
    let attachment = ImageAttachment {
        id: id(),
        name: crate::security::bounded(
            &name
                .chars()
                .filter(|c| !c.is_control() && *c != '/' && *c != '\\')
                .collect::<String>(),
            120,
        ),
        blob: store.blob(png.get_ref())?,
        original_blob: store.blob(bytes)?,
        width: normalized.width(),
        height: normalized.height(),
        bytes: png.get_ref().len() as u32,
        resized,
        source: if matches!(source, "clipboard" | "drop" | "browser" | "desktop") {
            source
        } else {
            "file"
        }
        .into(),
    };
    store.put("images", &attachment.id, &attachment)?;
    Ok(attachment)
}
pub fn preview(store: &Store, id: &str) -> Result<String> {
    let a: ImageAttachment = store.get("images", id)?;
    let image = image::load_from_memory(&store.bytes(&a.blob)?)
        .map_err(|_| AppError::new("IMAGE_CORRUPT", "This saved image could not be opened."))?
        .thumbnail(320, 220)
        .to_rgb8();
    let mut jpeg = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 75)
        .encode_image(&image)
        .map_err(|_| AppError::new("IMAGE_PREVIEW", "Image preview unavailable."))?;
    Ok(format!("data:image/jpeg;base64,{}", STANDARD.encode(jpeg)))
}
pub fn validate_inputs(store: &Store, inputs: &[ContentBlock]) -> Result<()> {
    if inputs
        .iter()
        .map(|b| {
            if let ContentBlock::Text { text } = b {
                text.len()
            } else {
                0
            }
        })
        .sum::<usize>()
        > 16 * 1024
    {
        return Err(AppError::new(
            "TASK_INPUT",
            "Combined text inputs must fit within 16,000 characters.",
        ));
    }
    let images = inputs
        .iter()
        .filter_map(|b| {
            if let ContentBlock::Image { attachment_id } = b {
                Some(attachment_id)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    if inputs.len() > 8 || images.len() > MAX_IMAGES {
        return Err(AppError::new(
            "IMAGE_COUNT",
            "Attach up to four images to one task.",
        ));
    }
    let mut seen = std::collections::HashSet::new();
    for id in images {
        if !seen.insert(id) {
            return Err(AppError::new(
                "IMAGE_DUPLICATE",
                "The same image is attached twice.",
            ));
        }
        let _: ImageAttachment = store.get("images", id)?;
    }
    for block in inputs {
        match block {
            ContentBlock::Text { text } if text.len() > 16000 => {
                return Err(AppError::new(
                    "TASK_INPUT",
                    "An input text block is too long.",
                ))
            }
            ContentBlock::FileReference { path } => {
                crate::security::relative_path(path)?;
            }
            _ => {}
        }
    }
    Ok(())
}
pub fn input_text(inputs: &[ContentBlock], fallback: &str) -> String {
    let blocks = inputs
        .iter()
        .filter_map(|b| match b {
            ContentBlock::Text { text } => Some(text.clone()),
            ContentBlock::FileReference { path } => {
                Some(format!("Referenced project file: {path}"))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    if blocks.is_empty() {
        fallback.into()
    } else {
        blocks.join("\n\n")
    }
}
pub fn ids(inputs: &[ContentBlock]) -> Vec<String> {
    inputs
        .iter()
        .filter_map(|b| {
            if let ContentBlock::Image { attachment_id } = b {
                Some(attachment_id.clone())
            } else {
                None
            }
        })
        .collect()
}
pub fn hydrate(store: &Store, messages: &[serde_json::Value]) -> Result<Vec<serde_json::Value>> {
    let mut out = messages.to_vec();
    for m in &mut out {
        if let Some(refs) = m.get("image_refs").and_then(|r| r.as_array()).cloned() {
            if refs.len() > MAX_IMAGES {
                return Err(AppError::new("IMAGE_COUNT", "Too many visual inputs."));
            }
            let mut images = Vec::new();
            for id in refs {
                let a: ImageAttachment = store.get("images", id.as_str().unwrap_or(""))?;
                images.push(STANDARD.encode(store.bytes(&a.blob)?));
            }
            m.as_object_mut().unwrap().remove("image_refs");
            m["images"] = serde_json::json!(images);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validated_encrypted_images_and_references() {
        let tmp = tempfile::tempdir().unwrap();
        let s = Store::open(tmp.path()).unwrap();
        assert!(ingest(&s, "bad.svg", b"<svg/>", "file").is_err());
        assert!(ingest(&s, "bad.png", b"\x89PNG\r\n", "file").is_err());
        assert!(ingest(&s, "huge.png", &vec![0; MAX_IMAGE_BYTES + 1], "file").is_err());
        let mut png = Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(80, 60)
            .write_to(&mut png, ImageFormat::Png)
            .unwrap();
        let a = ingest(&s, "screen.png", png.get_ref(), "clipboard").unwrap();
        assert_eq!((a.width, a.height), (80, 60));
        assert!(preview(&s, &a.id)
            .unwrap()
            .starts_with("data:image/jpeg;base64,"));
        let input = vec![
            ContentBlock::Text {
                text: "Inspect this".into(),
            },
            ContentBlock::Image {
                attachment_id: a.id.clone(),
            },
        ];
        validate_inputs(&s, &input).unwrap();
        let hydrated = hydrate(
            &s,
            &[serde_json::json!({"role":"user","content":"Look","image_refs":[a.id]})],
        )
        .unwrap();
        assert_eq!(hydrated[0]["images"].as_array().unwrap().len(), 1);
        assert!(hydrated[0].get("image_refs").is_none());
        assert!(validate_inputs(&s, &vec![input[1].clone(); 5]).is_err());
    }
    #[test]
    fn jpeg_webp_and_dimension_bounds() {
        let tmp = tempfile::tempdir().unwrap();
        let s = Store::open(tmp.path()).unwrap();
        for format in [ImageFormat::Jpeg, ImageFormat::WebP] {
            let mut data = Cursor::new(Vec::new());
            image::DynamicImage::new_rgb8(20, 30)
                .write_to(&mut data, format)
                .unwrap();
            let a = ingest(&s, "image", data.get_ref(), "file").unwrap();
            assert_eq!((a.width, a.height), (20, 30));
            assert_eq!(s.bytes(&a.original_blob).unwrap(), data.get_ref().clone());
        }
        let mut wide = Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(16001, 1)
            .write_to(&mut wide, ImageFormat::Png)
            .unwrap();
        assert_eq!(
            ingest(&s, "wide", wide.get_ref(), "file").unwrap_err().code,
            "IMAGE_DIMENSIONS"
        );
    }
}
