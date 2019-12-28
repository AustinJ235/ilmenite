use crate::ImtParser;
use crate::ImtShaper;
use crate::ImtRaster;
use crate::ImtError;
use crate::ImtRasterOps;
use crate::ImtGlyph;
use crate::ImtShapeOpts;
use std::path::Path;

#[derive(Debug,Clone,PartialEq,Hash,Eq)]
pub enum ImtWeight {
	Thin,
	ExtraLight,
	Light,
	Normal,
	Medium,
	SemiBold,
	Bold,
	ExtraBold,
	UltraBold
}

enum ImtFontByteSrc {
	Owned(Vec<u8>),
	Borrow(&'static [u8]),
}

#[derive(Debug,Clone,PartialEq,Hash,Eq)]
pub(crate) struct ImtFontKey {
	pub family: String,
	pub weight: ImtWeight,
}

pub struct ImtFont {
	family: String,
	source: ImtFontByteSrc,
	weight: ImtWeight,
	parser: ImtParser,
	shaper: ImtShaper,
	raster: ImtRaster,
}

impl ImtFont {
	pub fn from_file<F: Into<String>, P: AsRef<Path>>(
		family: F,
		weight: ImtWeight,
		raster_ops: ImtRasterOps,
		path: P
	) -> Result<ImtFont, ImtError> {
		unimplemented!()
	}
	
	pub fn from_bytes_owned<F: Into<String>>(
		family: F,
		weight: ImtWeight,
		raster_ops: ImtRasterOps,
		bytes: Vec<u8>
	) -> Result<ImtFont, ImtError> {
		unimplemented!()
	}
	
	pub fn from_bytes<F: Into<String>>(
		family: F,
		weight: ImtWeight,
		raster_ops: ImtRasterOps,
		bytes: &'static [u8]
	) -> Result<ImtFont, ImtError> {
		unimplemented!()
	}
	
	pub(crate) fn key(&self) -> ImtFontKey {
		ImtFontKey {
			family: self.family.clone(),
			weight: self.weight.clone()
		}
	}
	
	pub fn glyphs_for_text<T: AsRef<str>>(
		&mut self,
		text_height: f32,
		shape_ops: ImtShapeOpts,
		text: T,
	) -> Result<Vec<ImtGlyph>, ImtError> {
		unimplemented!()
	}
}
