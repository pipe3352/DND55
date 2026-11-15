//! Portrait/asset store. Uploaded images are untrusted: they are decoded, size-limited, resized and
//! re-encoded as PNG (which drops any embedded metadata) before being stored and shared.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub const MAX_UPLOAD_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_SOURCE_DIMENSION: u32 = 4096;
pub const PORTRAIT_SIZE: u32 = 256;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AssetError {
    #[error("file too large (max 4 MB)")]
    TooLarge,
    #[error("not a valid PNG image")]
    NotPng,
    #[error("image dimensions too large")]
    TooBig,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetStore {
    /// sha256 hex → PNG bytes.
    pub blobs: BTreeMap<String, Vec<u8>>,
}

impl AssetStore {
    /// Validates and normalizes a portrait, returning its content hash.
    pub fn add_portrait(&mut self, png: &[u8]) -> Result<String, AssetError> {
        if png.len() > MAX_UPLOAD_BYTES {
            return Err(AssetError::TooLarge);
        }
        if !png.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
            return Err(AssetError::NotPng);
        }
        let img = image::load_from_memory_with_format(png, image::ImageFormat::Png)
            .map_err(|_| AssetError::NotPng)?;
        if img.width() > MAX_SOURCE_DIMENSION
            || img.height() > MAX_SOURCE_DIMENSION
            || img.width() == 0
            || img.height() == 0
        {
            return Err(AssetError::TooBig);
        }
        let img = if img.width() > PORTRAIT_SIZE || img.height() > PORTRAIT_SIZE {
            img.thumbnail(PORTRAIT_SIZE, PORTRAIT_SIZE)
        } else {
            img
        };
        let rgba = image::DynamicImage::ImageRgba8(img.to_rgba8());
        let mut out = std::io::Cursor::new(Vec::new());
        rgba.write_to(&mut out, image::ImageFormat::Png)
            .map_err(|_| AssetError::NotPng)?;
        let bytes = out.into_inner();
        let hash = hex::encode(Sha256::digest(&bytes));
        self.blobs.insert(hash.clone(), bytes);
        Ok(hash)
    }

    pub fn get(&self, hash: &str) -> Option<&[u8]> {
        self.blobs.get(hash).map(|v| v.as_slice())
    }
}

#[cfg(test)]
pub(crate) fn test_png(w: u32, h: u32) -> Vec<u8> {
    let img = image::RgbaImage::from_fn(w, h, |x, y| {
        image::Rgba([(x % 256) as u8, (y % 256) as u8, 128, 255])
    });
    let mut out = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut out, image::ImageFormat::Png)
        .unwrap();
    out.into_inner()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_and_normalizes_png() {
        let mut s = AssetStore::default();
        let h = s.add_portrait(&test_png(600, 300)).unwrap();
        let img = image::load_from_memory(s.get(&h).unwrap()).unwrap();
        assert_eq!((img.width(), img.height()), (256, 128));
    }

    #[test]
    fn rejects_garbage() {
        let mut s = AssetStore::default();
        assert_eq!(s.add_portrait(b"GIF89a...."), Err(AssetError::NotPng));
        let mut fake = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        fake.extend([0u8; 64]);
        assert_eq!(s.add_portrait(&fake), Err(AssetError::NotPng));
        assert_eq!(
            s.add_portrait(&vec![0u8; MAX_UPLOAD_BYTES + 1]),
            Err(AssetError::TooLarge)
        );
    }
}
