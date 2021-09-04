//! ```rust
//! let ilmenite = Ilmenite::new();
//!
//! ilmenite.add_font(
//!     ImtFont::from_file(
//!         "MyFont",
//!         ImtWeight::Normal,
//!         ImtRasterOpts::default(),
//!         device,
//!         queue,
//!         "MyFont.ttf",
//!     )
//!     .unwrap(),
//! );
//!
//! let glyphs = ilmenite
//!     .glyphs_for_text("MyFont", ImtWeight::Normal, 12.0, None, "Hello World!")
//!     .unwrap();
//! ```

extern crate allsorts;
extern crate crossbeam;
extern crate ordered_float;
extern crate parking_lot;
extern crate vulkano;
extern crate vulkano_shaders;

pub mod bitmap;
pub mod error;
pub mod font;
pub mod image_view;
pub mod parse;
pub mod primative;
pub mod raster;
pub mod script;
pub mod shaders;
pub mod shape;

pub use bitmap::{ImtBitmapData, ImtGlyphBitmap};
use crossbeam::sync::ShardedLock;
pub use error::{ImtError, ImtErrorSrc, ImtErrorTy};
pub(crate) use font::ImtFontKey;
pub use font::{ImtFont, ImtWeight};
pub use image_view::ImtImageView;
pub use parse::{ImtFontProps, ImtParsedGlyph, ImtParser};
pub use primative::{ImtGeometry, ImtPoint, ImtPosition};
pub use raster::{
    ImtFillQuality, ImtRaster, ImtRasterOpts, ImtRasteredGlyph, ImtSampleQuality,
};
pub use script::{ImtLang, ImtScript};
pub use shape::{
    ImtGlyphInfo, ImtHoriAlign, ImtShapeOpts, ImtShapedGlyph, ImtShaper, ImtTextWrap,
    ImtVertAlign,
};
use std::collections::HashMap;
use vulkano::device::Features as VkFeatures;

pub fn ilmenite_required_vk_features() -> VkFeatures {
    VkFeatures {
        shader_storage_image_write_without_format: true,
        .. VkFeatures::none()
    }
}

pub struct ImtGlyph {
    pub x: f32,
    pub y: f32,
    pub w: u32,
    pub h: u32,
    pub crop_x: f32,
    pub crop_y: f32,
    pub family: String,
    pub weight: ImtWeight,
    pub index: u16,
    pub bitmap: Option<ImtBitmapData>,
}

pub struct Ilmenite {
    fonts: ShardedLock<HashMap<ImtFontKey, ImtFont>>,
}

impl Ilmenite {
    pub fn new() -> Self {
        Ilmenite {
            fonts: ShardedLock::new(HashMap::new()),
        }
    }

    pub fn add_font(&self, font: ImtFont) {
        let key = font.key();
        self.fonts.write().unwrap().insert(key, font);
    }

    pub fn glyphs_for_text<T: AsRef<str>>(
        &self,
        family: String,
        weight: ImtWeight,
        text_height: f32,
        shape_ops: Option<ImtShapeOpts>,
        text: T,
    ) -> Result<Vec<ImtGlyph>, ImtError> {
        self.fonts
            .read()
            .unwrap()
            .get(&ImtFontKey {
                family,
                weight,
            })
            .ok_or(ImtError::src_and_ty(ImtErrorSrc::Ilmenite, ImtErrorTy::MissingFont))?
            .glyphs_for_text(text_height, shape_ops.unwrap_or(ImtShapeOpts::default()), text)
    }
}
