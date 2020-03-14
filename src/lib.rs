extern crate vulkano;
#[macro_use]
extern crate vulkano_shaders;
extern crate allsorts;
extern crate ordered_float;
extern crate parking_lot;

pub mod bitmap;
pub mod error;
pub mod font;
pub mod parse;
pub mod primative;
pub mod raster;
pub mod script;
pub mod shaders;
pub mod shape;

pub use bitmap::ImtGlyphBitmap;
pub use error::ImtError;
pub use error::ImtErrorSrc;
pub use error::ImtErrorTy;
pub use font::ImtFont;
pub use font::ImtWeight;
pub use parse::ImtFontProps;
pub use parse::ImtParsedGlyph;
pub use parse::ImtParser;
pub use primative::ImtGeometry;
pub use primative::ImtPoint;
pub use primative::ImtPosition;
pub use raster::ImtFillQuality;
pub use raster::ImtRaster;
pub use raster::ImtRasterOps;
pub use raster::ImtRasteredGlyph;
pub use raster::ImtSampleQuality;
pub use script::ImtLang;
pub use script::ImtScript;
pub use shape::ImtGlyphInfo;
pub use shape::ImtHoriAlign;
pub use shape::ImtShapeOpts;
pub use shape::ImtShapedGlyph;
pub use shape::ImtShaper;
pub use shape::ImtTextWrap;
pub use shape::ImtVertAlign;

pub(crate) use font::ImtFontKey;
pub(crate) use primative::ImtShaderVert;

use parking_lot::Mutex;
use std::collections::HashMap;

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
    pub bitmap: Vec<f32>,
}

pub struct Ilmenite {
    fonts: Mutex<HashMap<ImtFontKey, ImtFont>>,
}

impl Ilmenite {
    pub fn new() -> Self {
        Ilmenite {
            fonts: Mutex::new(HashMap::new()),
        }
    }

    pub fn add_font(&self, font: ImtFont) {
        let key = font.key();
        self.fonts.lock().insert(key, font);
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
            .lock()
            .get_mut(&ImtFontKey { family, weight })
            .ok_or(ImtError::src_and_ty(
                ImtErrorSrc::Ilmenite,
                ImtErrorTy::MissingFont,
            ))?
            .glyphs_for_text(
                text_height,
                shape_ops.unwrap_or(ImtShapeOpts::default()),
                text,
            )
    }
}
