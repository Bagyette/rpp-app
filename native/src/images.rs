//! Shared resource limits for imported and embedded raster images.

use std::io::Cursor;

use image::{DynamicImage, ImageError, ImageReader};

pub const MAX_ENCODED_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_DIMENSION: u32 = 8192;
pub const MAX_DECODED_BYTES: u64 = 256 * 1024 * 1024;

pub fn reader(bytes: &[u8]) -> Result<ImageReader<Cursor<&[u8]>>, ImageError> {
    let mut reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_DIMENSION);
    limits.max_image_height = Some(MAX_DIMENSION);
    limits.max_alloc = Some(MAX_DECODED_BYTES);
    reader.limits(limits);
    Ok(reader)
}

pub fn decode(bytes: &[u8]) -> Result<DynamicImage, ImageError> {
    reader(bytes)?.decode()
}

pub fn decoded_byte_len(bytes: &[u8]) -> Option<u64> {
    let (width, height) = reader(bytes).ok()?.into_dimensions().ok()?;
    let bytes = u64::from(width) * u64::from(height) * 4;
    (width > 0
        && height > 0
        && width <= MAX_DIMENSION
        && height <= MAX_DIMENSION
        && bytes <= MAX_DECODED_BYTES)
        .then_some(bytes)
}

pub fn valid_dimensions(bytes: &[u8]) -> bool {
    decoded_byte_len(bytes).is_some()
}

#[cfg(test)]
mod tests {
    #[test]
    fn oversized_image_headers_are_rejected_before_allocating_pixels() {
        // BMP dimensions can be checked without constructing its pixel buffer.
        let mut bmp = vec![0_u8; 54];
        bmp[..2].copy_from_slice(b"BM");
        bmp[10..14].copy_from_slice(&54_u32.to_le_bytes());
        bmp[14..18].copy_from_slice(&40_u32.to_le_bytes());
        bmp[18..22].copy_from_slice(&100_000_u32.to_le_bytes());
        bmp[22..26].copy_from_slice(&100_000_u32.to_le_bytes());
        bmp[26..28].copy_from_slice(&1_u16.to_le_bytes());
        bmp[28..30].copy_from_slice(&24_u16.to_le_bytes());
        assert!(!super::valid_dimensions(&bmp));
        assert!(super::decode(&bmp).is_err());
    }
}
