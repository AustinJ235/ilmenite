#[derive(Clone,Debug,PartialEq)]
pub enum ImtVertAlign {
	Top,
	Bottom,
	Center,
}

#[derive(Clone,Debug,PartialEq)]
pub enum ImtHoriAlign {
	Left,
	Right,
	Center,
}

#[derive(Clone,Debug,PartialEq)]
pub enum ImtTextWrap {
	Shift,
	NewLine,
	None,
}

#[derive(Clone,Debug,PartialEq)]
pub enum ItmPixelAlign {
	Care,
	DontCare,
}

#[derive(Clone,Debug,PartialEq)]
pub struct ItmShapeOpts {
	pub body_width: f32,
	pub body_height: f32,
	pub text_height: f32,
	pub text_wrap: ImtTextWrap,
	pub vert_align: ImtVertAlign,
	pub hori_align: ImtHoriAlign,
	pub pixel_align: ItmPixelAlign,
}

#[derive(Clone,Debug)]
pub struct ImtGlyphInfo {
	pub font_family: String,
	pub text_height: f32,
	pub word_index: usize,
	pub line_index: usize,
	pub pos_from_l: Option<f32>,
	pub pos_from_r: Option<f32>,
	pub pos_from_t: Option<f32>,
	pub pos_from_b: Option<f32>,
}
