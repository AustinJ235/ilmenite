extern crate vulkano;
#[macro_use]
extern crate vulkano_shaders;
extern crate allsorts;
extern crate ordered_float;
extern crate parking_lot;

pub mod error;
pub mod script;
pub mod primative;
pub mod parse;
pub mod shape;
pub mod raster;
pub mod bitmap;
pub mod font;
mod shaders;

pub use error::ImtError;
pub use error::ImtErrorTy;
pub use error::ImtErrorSrc;
pub use script::ImtScript;
pub use script::ImtLang;
pub use primative::ImtGeometry;
pub use primative::ImtPosition;
pub use primative::ImtPoint;
pub(crate) use primative::ImtShaderVert;
pub use parse::ImtParser;
pub use parse::ImtParsedGlyph;
pub use parse::ImtFontProps;
pub use shape::ImtVertAlign;
pub use shape::ImtHoriAlign;
pub use shape::ImtTextWrap;
pub use shape::ImtPixelAlign;
pub use shape::ImtShapeOpts;
pub use shape::ImtGlyphInfo;
pub use shape::ImtShapedGlyph;
pub use shape::ImtShaper;
pub use raster::ImtRaster;
pub use raster::ImtFillQuality;
pub use raster::ImtSampleQuality;
pub use raster::ImtRasterOps;
pub use raster::ImtRasteredGlyph;
pub use bitmap::ImtGlyphBitmap;
pub use font::ImtFont;
pub use font::ImtWeight;
pub(crate) use font::ImtFontKey;

use parking_lot::Mutex;
use std::collections::HashMap;

pub struct ImtGlyph {
	pub x: f32,
	pub y: f32,
	pub w: u32,
	pub h: u32,
	pub bitmap: Vec<f32>,
}

pub struct Ilmenite {
	fonts: Mutex<HashMap<ImtFontKey, Mutex<ImtFont>>>,
}

impl Ilmenite {
	pub fn new() -> Self {
		Ilmenite {
			fonts: Mutex::new(HashMap::new()),
		}
	}
	
	pub fn add_font(&self, font: ImtFont) {
		let key = font.key();
		self.fonts.lock().insert(key, Mutex::new(font));
	}
	
	pub fn glyphs_for_text<T: AsRef<str>>(
		&self,
		family: String,
		weight: ImtWeight,
		text_height: f32,
		shape_ops: Option<ImtShapeOpts>,
		text: T
	) -> Result<Vec<ImtGlyph>, ImtError> {
		unimplemented!()
	}
}
