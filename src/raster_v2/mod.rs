// TODO: Feature Block
pub mod gpu;

use std::sync::Arc;

// TODO: Feature Block
pub use gpu::ImtRasterGpu;
use vulkano::format::Format;

use crate::{ImtError, ImtImageView, ImtParser, ImtShapedGlyph};

/// Rasterization options use to raster glyphs.
pub struct ImtRasterOps {
    pub ssaa: ImtSSAA,
    pub subpixel: ImtSubPixel,
    // TODO: Feature Block
    pub bitmap_format: Format,
}

/// Amount of samples to use per subpixel
///
/// If `ImtSubPixel` is `ImtSubPixel::None` then this is per pixel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImtSSAA {
    /// 1 Sample
    X1,
    /// 4 Samples
    X2,
    /// 9 Samples
    X3,
    /// 16 samples
    #[default]
    X4,
    /// 25 samples
    X5,
    /// 36 samples
    X6,
    /// 49 samples
    X7,
    /// 64 samples
    X8,
}

impl ImtSSAA {
    pub(in crate::raster_v2) fn as_uint(self) -> u32 {
        match self {
            Self::X1 => 1,
            Self::X2 => 2,
            Self::X3 => 3,
            Self::X4 => 4,
            Self::X5 => 5,
            Self::X6 => 6,
            Self::X7 => 7,
            Self::X8 => 8,
        }
    }
}

/// The subpixel layout of a pixel.
///
/// This is most commonly `RGB`, but if a display but could be differ
/// between displays or even the same display depending on orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImtSubPixel {
    /// Don't use subpixel hinting.
    None,
    /// Most common subpixel layout.
    #[default]
    RGB,
    /// Most commonly a display rotated clockwise to portrait mode.
    VRGB,
    /// Most commonly a display being upside down.
    BGR,
    /// Most commonly a display rotated counter-clockwise to portrait mode.
    VBGR,
}

impl ImtSubPixel {
    pub(in crate::raster_v2) fn as_uint(self) -> u32 {
        match self {
            Self::None => 0,
            Self::RGB => 1,
            Self::VRGB => 2,
            Self::BGR => 3,
            Self::VBGR => 4,
        }
    }
}

/// The output of `raster_shaped_glyphs`.
pub struct ImtRasteredGlyph {
    pub shaped: ImtShapedGlyph,
    pub bitmap: Arc<ImtGlyphBitmap>,
}

pub struct ImtGlyphBitmap {
    pub width: u32,
    pub height: u32,
    pub bearing_x: f32,
    pub bearing_y: f32,
    pub text_height: f32,
    pub glyph_index: u16,
    pub data: ImtBitmapData,
}

#[derive(Clone)]
pub enum ImtBitmapData {
    Empty,
    LRGBA(Arc<Vec<f32>>),
    // TODO: Feature Block
    Image(Arc<ImtImageView>),
}

pub trait ImtRaster {
    fn raster_shaped_glyphs(
        &self,
        parser: &ImtParser,
        text_height: f32,
        shaped_glyphs: Vec<ImtShapedGlyph>,
    ) -> Result<Vec<ImtRasteredGlyph>, ImtError>;
}
