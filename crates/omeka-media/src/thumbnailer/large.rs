//! A reduced copy of an original too large to hold whole (over 64 MB,
//! `docs/import-v2.md`, "Bytes"), made with bounded memory.
//!
//! A TIFF is read one chunk (strip or tile) at a time and box-averaged into
//! a copy whose longer side is at most [`REDUCED`]; the memory is that copy's
//! sums plus one chunk. A baseline JPEG is decoded with IDCT scaling, so only
//! a reduced copy is ever made. The three derivatives are then made from the
//! copy as from any image.

use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};

use image::metadata::Orientation;
use image::{DynamicImage, RgbImage};
use tiff::ColorType;
use tiff::decoder::{ChunkType, Decoder, DecodingResult, Limits};

use super::ThumbnailSkip;

/// The longer side of the reduced copy: twice the largest derivative, so
/// the final resize still has pixels to average.
pub const REDUCED: u32 = 1600;

/// The largest decoded chunk read: a TIFF whose strip is larger (one strip
/// for the whole image, compressed) is not thumbnailed.
pub const MAX_CHUNK_BYTES: usize = 32 * 1024 * 1024;

/// The largest coefficient store a progressive JPEG may need: it keeps every
/// coefficient of the full image, whatever the scale.
const MAX_PROGRESSIVE_BYTES: usize = 64 * 1024 * 1024;

fn decode_error(error: impl std::fmt::Display) -> ThumbnailSkip {
    ThumbnailSkip::Decode(error.to_string())
}

/// The reduced copy of a TIFF's first image.
pub(super) fn tiff<R: Read + Seek>(reader: R) -> Result<DynamicImage, ThumbnailSkip> {
    let limits = {
        let mut limits = Limits::default();
        limits.decoding_buffer_size = MAX_CHUNK_BYTES;
        limits.intermediate_buffer_size = MAX_CHUNK_BYTES;
        limits
    };
    let mut decoder = Decoder::new(reader)
        .map_err(decode_error)?
        .with_limits(limits);
    let (width, height) = decoder.dimensions().map_err(decode_error)?;
    if width == 0 || height == 0 {
        return Err(ThumbnailSkip::Decode("the image has no pixels".to_string()));
    }
    let orientation = decoder
        .find_tag_unsigned::<u16>(tiff::tags::Tag::Orientation)
        .ok()
        .flatten()
        .and_then(|value| Orientation::from_exif(value.min(255) as u8))
        .unwrap_or(Orientation::NoTransforms);
    let layout = Layout::of(decoder.colortype().map_err(decode_error)?)?;
    let mut copy = Reduced::new(width, height);
    let (chunk_width, chunk_height) = decoder.chunk_dimensions();
    let (chunks, across) = match decoder.get_chunk_type() {
        ChunkType::Strip => (decoder.strip_count().map_err(decode_error)?, 1),
        ChunkType::Tile => (
            decoder.tile_count().map_err(decode_error)?,
            width.div_ceil(chunk_width.max(1)),
        ),
    };
    for chunk in 0..chunks {
        let (data_width, data_height) = decoder.chunk_data_dimensions(chunk);
        let left = (chunk % across) * chunk_width;
        let top = (chunk / across) * chunk_height;
        let samples = match decoder.read_chunk(chunk).map_err(decode_error)? {
            DecodingResult::U8(samples) => samples,
            DecodingResult::U16(samples) => samples.into_iter().map(|v| (v >> 8) as u8).collect(),
            _ => {
                return Err(ThumbnailSkip::Decode(
                    "a sample format with no thumbnail".into(),
                ));
            }
        };
        let row_bytes = layout.row_bytes(data_width);
        if samples.len() < row_bytes * data_height as usize {
            return Err(ThumbnailSkip::Decode("separate colour planes".to_string()));
        }
        for (y, row) in (top..).zip(samples.chunks_exact(row_bytes).take(data_height as usize)) {
            for x in 0..data_width {
                copy.add(left + x, y, layout.rgb(row, x as usize));
            }
        }
    }
    let mut image = DynamicImage::ImageRgb8(copy.finish());
    image.apply_orientation(orientation);
    Ok(image)
}

/// The reduced copy of a JPEG, decoded at the smallest IDCT scale whose
/// longer side still reaches [`REDUCED`].
pub(super) fn jpeg<R: Read + Seek>(reader: R) -> Result<DynamicImage, ThumbnailSkip> {
    use jpeg_decoder::{CodingProcess, PixelFormat};
    let mut reader = BufReader::new(reader);
    let orientation = exif_orientation(&mut reader);
    reader.seek(SeekFrom::Start(0)).map_err(decode_error)?;
    let mut decoder = jpeg_decoder::Decoder::new(reader);
    decoder.read_info().map_err(decode_error)?;
    let info = decoder
        .info()
        .ok_or_else(|| decode_error("no frame header"))?;
    let components = info.pixel_format.pixel_bytes();
    if info.coding_process == CodingProcess::DctProgressive
        && usize::from(info.width) * usize::from(info.height) * components * 2
            > MAX_PROGRESSIVE_BYTES
    {
        return Err(ThumbnailSkip::Decode(
            "a progressive JPEG this large keeps every coefficient in memory".to_string(),
        ));
    }
    let wanted = |side: u16| u16::try_from(u32::from(side).min(REDUCED)).unwrap_or(u16::MAX);
    let (longer, shorter) = (info.width.max(info.height), info.width.min(info.height));
    let scale = f64::from(wanted(longer)) / f64::from(longer);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let shorter = ((f64::from(shorter) * scale).ceil() as u16).max(1);
    let (asked_width, asked_height) = if info.width >= info.height {
        (wanted(longer), shorter)
    } else {
        (shorter, wanted(longer))
    };
    let (width, height) = decoder
        .scale(asked_width, asked_height)
        .map_err(decode_error)?;
    let pixels = decoder.decode().map_err(decode_error)?;
    let (width, height) = (u32::from(width), u32::from(height));
    let rgb: Vec<u8> = match info.pixel_format {
        PixelFormat::RGB24 => pixels,
        PixelFormat::L8 => pixels.into_iter().flat_map(|v| [v, v, v]).collect(),
        PixelFormat::L16 => pixels
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|two| [two[0], two[0], two[0]])
            .collect(),
        PixelFormat::CMYK32 => pixels
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|pixel| cmyk(pixel))
            .collect(),
    };
    let mut image = RgbImage::from_raw(width, height, rgb)
        .map(DynamicImage::ImageRgb8)
        .ok_or_else(|| decode_error("the scaled JPEG is short of pixels"))?;
    image.apply_orientation(orientation);
    Ok(image)
}

/// The EXIF orientation of a JPEG read from the start of `reader`. A missing
/// or malformed block leaves the pixels as stored.
fn exif_orientation<R: BufRead + Seek>(reader: &mut R) -> Orientation {
    exif::Reader::new()
        .read_from_container(reader)
        .ok()
        .and_then(|exif| {
            exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY)
                .and_then(|field| field.value.get_uint(0))
        })
        .and_then(|value| Orientation::from_exif(value.min(255) as u8))
        .unwrap_or(Orientation::NoTransforms)
}

fn cmyk(pixel: &[u8]) -> [u8; 3] {
    let k = u16::from(255 - pixel[3]);
    let channel = |c: u8| ((u16::from(255 - c) * k) / 255) as u8;
    [channel(pixel[0]), channel(pixel[1]), channel(pixel[2])]
}

/// How one pixel of a decoded TIFF chunk turns to RGB.
#[derive(Clone, Copy)]
enum Layout {
    /// One bit a pixel, packed, rows padded to a byte (bilevel scans).
    Bits,
    Gray {
        samples: usize,
    },
    Rgb {
        samples: usize,
    },
    Cmyk {
        samples: usize,
    },
}

impl Layout {
    /// The layouts the reduced path reads; samples are 8 bits after the
    /// 16-bit ones are cut to their high byte.
    fn of(color: ColorType) -> Result<Self, ThumbnailSkip> {
        Ok(match color {
            ColorType::Gray(1) => Self::Bits,
            ColorType::Gray(8 | 16) => Self::Gray { samples: 1 },
            ColorType::GrayA(8 | 16) => Self::Gray { samples: 2 },
            ColorType::RGB(8 | 16) => Self::Rgb { samples: 3 },
            ColorType::RGBA(8 | 16) => Self::Rgb { samples: 4 },
            ColorType::CMYK(8) => Self::Cmyk { samples: 4 },
            ColorType::CMYKA(8) => Self::Cmyk { samples: 5 },
            other => {
                return Err(ThumbnailSkip::Decode(format!(
                    "a {other:?} TIFF has no thumbnail when it is over 64 MB"
                )));
            }
        })
    }

    fn row_bytes(self, width: u32) -> usize {
        match self {
            Self::Bits => width.div_ceil(8) as usize,
            Self::Gray { samples } | Self::Rgb { samples } | Self::Cmyk { samples } => {
                width as usize * samples
            }
        }
    }

    fn rgb(self, row: &[u8], x: usize) -> [u8; 3] {
        match self {
            Self::Bits => {
                let on = row[x / 8] & (0x80 >> (x % 8)) != 0;
                if on { [255; 3] } else { [0; 3] }
            }
            Self::Gray { samples } => [row[x * samples]; 3],
            Self::Rgb { samples } => {
                let at = x * samples;
                [row[at], row[at + 1], row[at + 2]]
            }
            Self::Cmyk { samples } => cmyk(&row[x * samples..x * samples + 4]),
        }
    }
}

/// A box-averaged copy: each source pixel adds into the copy's pixel it
/// falls in, and each copy pixel is the mean of what fell in it.
struct Reduced {
    width: u32,
    height: u32,
    /// The copy pixel each source column and row falls in.
    column_bin: Vec<u32>,
    row_bin: Vec<u32>,
    sums: Vec<u32>,
}

impl Reduced {
    fn new(width: u32, height: u32) -> Self {
        let longer = width.max(height);
        let fit = |side: u32| {
            if longer <= REDUCED {
                side
            } else {
                (u64::from(side) * u64::from(REDUCED)).div_ceil(u64::from(longer)) as u32
            }
        };
        let (copy_width, copy_height) = (fit(width).max(1), fit(height).max(1));
        let bins = |side: u32, copy: u32| -> Vec<u32> {
            (0..side)
                .map(|at| (u64::from(at) * u64::from(copy) / u64::from(side)) as u32)
                .collect()
        };
        Self {
            column_bin: bins(width, copy_width),
            row_bin: bins(height, copy_height),
            width: copy_width,
            height: copy_height,
            sums: vec![0; copy_width as usize * copy_height as usize * 3],
        }
    }

    fn add(&mut self, x: u32, y: u32, rgb: [u8; 3]) {
        let (Some(&column), Some(&row)) = (
            self.column_bin.get(x as usize),
            self.row_bin.get(y as usize),
        ) else {
            return;
        };
        let at = (row as usize * self.width as usize + column as usize) * 3;
        for (sum, value) in self.sums[at..at + 3].iter_mut().zip(rgb) {
            *sum += u32::from(value);
        }
    }

    fn finish(self) -> RgbImage {
        let count = |bins: &[u32], copy: u32| {
            let mut counts = vec![0_u32; copy as usize];
            for bin in bins {
                counts[*bin as usize] += 1;
            }
            counts
        };
        let columns = count(&self.column_bin, self.width);
        let rows = count(&self.row_bin, self.height);
        RgbImage::from_fn(self.width, self.height, |x, y| {
            let pixels = (columns[x as usize] * rows[y as usize]).max(1);
            let at = (y as usize * self.width as usize + x as usize) * 3;
            let mean = |channel: usize| (self.sums[at + channel] / pixels) as u8;
            image::Rgb([mean(0), mean(1), mean(2)])
        })
    }
}
