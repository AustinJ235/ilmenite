use crate::ImtParser;
use crate::ImtParsedGlyph;
use crate::ImtError;
use crate::ImtErrorSrc;
use crate::ImtErrorTy;
use crate::ImtPosition;
use crate::ImtScript;
use crate::ImtLang;

use allsorts::gpos::{gpos_apply,Info,Placement};
use std::sync::Arc;
use std::rc::Rc;

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
pub struct ImtShapeOpts {
	pub body_width: f32,
	pub body_height: f32,
	pub text_height: f32,
	pub text_wrap: ImtTextWrap,
	pub vert_align: ImtVertAlign,
	pub hori_align: ImtHoriAlign,
}

impl Default for ImtShapeOpts {
	fn default() -> Self {
		ImtShapeOpts {
			body_width: 0.0,
			body_height: 0.0,
			text_height: 36.0,
			text_wrap: ImtTextWrap::None,
			vert_align: ImtVertAlign::Top,
			hori_align: ImtHoriAlign::Left,
		}
	}
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

pub struct ImtShapedGlyph {
	pub parsed: Arc<ImtParsedGlyph>,
	pub position: ImtPosition,
}

pub struct ImtShaper {

}

impl ImtShaper {
	pub fn new() -> Result<Self, ImtError> {
		Ok(ImtShaper {
		
		})
	}
	
	pub fn shape_parsed_glyphs(
		&self,
		parser: &mut ImtParser,
		script: ImtScript,
		lang: ImtLang,
		_opts: ImtShapeOpts,
		glyphs: Vec<Arc<ImtParsedGlyph>>
	) -> Result<Vec<ImtShapedGlyph>, ImtError> {
	
		let mut imt_shaped_glyphs = Vec::new();
		let mut raw_glyphs = Vec::new();
	
		for parsed_glyph in glyphs {
			raw_glyphs.push(parsed_glyph.inner.clone());
			
			imt_shaped_glyphs.push(ImtShapedGlyph {
				parsed: parsed_glyph,
				position: ImtPosition { x: 0.0, y: 0.0 },
			});
		}
		
		let mut infos = Info::init_from_glyphs(parser.gdef_op.as_ref(), raw_glyphs)
			.map_err(|e| ImtError::allsorts_parse(ImtErrorSrc::GsubInfo, e))?;
			
		if let Some(gpos) = parser.gpos_op.take() {
			let gpos_rc = Rc::new(gpos);
			
			gpos_apply(
				&gpos_rc,
				parser.gdef_op.as_ref(),
				true,
				script.tag(),
				lang.tag(),
				&mut infos
			).map_err(|e| ImtError::allsorts_parse(ImtErrorSrc::GPOS, e))?;
			
			parser.gpos_op = Some(Rc::try_unwrap(gpos_rc).ok().unwrap());
		}
		
		let mut x: f32 = 0.0;
		let y: f32 = 0.0;
		
		for (i, info) in infos.into_iter().enumerate() {
			let glyph_index = info.glyph.glyph_index
				.ok_or(ImtError::src_and_ty(ImtErrorSrc::Glyph, ImtErrorTy::MissingIndex))?;
			let hori_adv = parser.hmtx.horizontal_advance(glyph_index, parser.hhea.num_h_metrics)
				.map_err(|e| ImtError::allsorts_parse(ImtErrorSrc::Glyph, e))? as f32;
			
			let (glyph_x, glyph_y) = match info.placement {
				Placement::Distance(dist_x, dist_y) => {
					let dist_x = dist_x as f32;
					let dist_y = dist_y as f32;
					(x + dist_x, y + dist_y)
				},
				Placement::Anchor(_, _) | Placement::None => {
					(x, y)
				}
			};
			
			imt_shaped_glyphs[i].position = ImtPosition {
				x: glyph_x,
				y: glyph_y
			};
			
			x += hori_adv;
		}
		
		Ok(imt_shaped_glyphs)
	}
}
