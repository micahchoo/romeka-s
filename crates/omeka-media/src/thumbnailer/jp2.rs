//! JPEG 2000 originals (`image/jp2`), decoded with `hayro-jpeg2000`.
//!
//! JPEG 2000 stores each image at halving resolutions, so the decoder skips
//! the levels finer than the largest derivative needs: a 10,000-pixel master
//! is decoded at about 1,250 pixels, a sixty-fourth of its samples.

use hayro_jpeg2000::{ColorSpace, DecodeSettings, DecoderContext, Image};
use image::{DynamicImage, GrayAlphaImage, GrayImage, RgbImage, RgbaImage};

use super::ThumbnailSkip;
use super::large::REDUCED;

pub(super) fn decode(bytes: &[u8]) -> Result<DynamicImage, ThumbnailSkip> {
    let error = |error: hayro_jpeg2000::DecodeError| ThumbnailSkip::Decode(error.to_string());
    let settings = DecodeSettings {
        target_resolution: Some((REDUCED, REDUCED)),
        ..DecodeSettings::default()
    };
    let image = Image::new(bytes, &settings).map_err(error)?;
    let (width, height) = (image.width(), image.height());
    let alpha = image.has_alpha();
    let space = image.color_space().clone();
    let mut context = DecoderContext::default();
    let samples = image.decode(&mut context).map_err(error)?.data_u8();
    let short = || ThumbnailSkip::Decode("the decoded image is short of samples".to_string());
    // An ICC-tagged image is shown in its own primaries: the profile is
    // not applied, as the in-memory thumbnailer does not apply one either.
    let channels = match &space {
        ColorSpace::CMYK => 0,
        other => other.num_channels(),
    };
    Ok(match (channels, alpha) {
        (1, false) => GrayImage::from_raw(width, height, samples)
            .map(DynamicImage::ImageLuma8)
            .ok_or_else(short)?,
        (1, true) => GrayAlphaImage::from_raw(width, height, samples)
            .map(DynamicImage::ImageLumaA8)
            .ok_or_else(short)?,
        (3, false) => RgbImage::from_raw(width, height, samples)
            .map(DynamicImage::ImageRgb8)
            .ok_or_else(short)?,
        (3, true) => RgbaImage::from_raw(width, height, samples)
            .map(DynamicImage::ImageRgba8)
            .ok_or_else(short)?,
        _ => {
            return Err(ThumbnailSkip::Decode(format!(
                "a JPEG 2000 image in {space:?} has no thumbnail"
            )));
        }
    })
}
