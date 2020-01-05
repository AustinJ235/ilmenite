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
	NoneDotted,
}

#[derive(Clone,Debug,PartialEq)]
pub struct ImtShapeOpts {
	pub body_width: f32,
	pub body_height: f32,
	pub text_height: f32,
	pub line_spacing: f32,
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
			line_spacing: 0.0,
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
	pub x_overflow: f32,
	pub y_overflow: f32,
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
		opts: ImtShapeOpts,
		glyphs: Vec<Arc<ImtParsedGlyph>>
	) -> Result<Vec<ImtShapedGlyph>, ImtError> {
	
		let font_props = parser.font_props();
		let mut imt_shaped_glyphs: Vec<ImtShapedGlyph> = Vec::new();
		let mut raw_glyphs = Vec::new();
	
		for parsed_glyph in glyphs {
			raw_glyphs.push(parsed_glyph.inner.clone());
			
			imt_shaped_glyphs.push(ImtShapedGlyph {
				parsed: parsed_glyph,
				position: ImtPosition { x: 0.0, y: 0.0 },
				y_overflow: 0.0,
				x_overflow: 0.0,
			});
		}
		
		let mut shape_from = 0;
		let mut y = 0.0;
		let line_spacing = ((opts.text_height / 18.0).floor() + opts.line_spacing) / (font_props.scaler * opts.text_height);
		let vert_adv = font_props.line_gap + font_props.ascender + line_spacing;
		let mut lines: Vec<(usize, usize, f32)> = Vec::new();
		
		'line: loop {
			let mut infos = Info::init_from_glyphs(parser.gdef_op.as_ref(), raw_glyphs[shape_from..].to_vec())
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
			let mut x_offset = 0.0;
			let mut line_max_x = 0.0;
			let infos_len = infos.len();
			
			for (i, info) in infos.into_iter().enumerate() {
				if *info.glyph.unicodes.first().unwrap() == '\n' {
					if shape_from + i >= raw_glyphs.len() {
						break 'line;
					} else {
						if i == 0 {
							lines.push((shape_from, shape_from, line_max_x));
						} else {
							lines.push((shape_from, shape_from + i, line_max_x));
						}

						y += vert_adv;
						shape_from = shape_from + i + 1;
						continue 'line;
					}
				}
				
				if x == 0.0 {
					x_offset = imt_shaped_glyphs[i].parsed.min_x;
				}
				
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
				
				let lmaxx = glyph_x + x_offset + imt_shaped_glyphs[i + shape_from].parsed.max_x;
				
				if let &ImtTextWrap::NewLine = &opts.text_wrap {
					if lmaxx * font_props.scaler * opts.text_height > opts.body_width {
						lines.push((shape_from, i + shape_from, line_max_x));
						shape_from += i;
						y += vert_adv;
						continue 'line;
					}
				}
				
				line_max_x = lmaxx;
				
				imt_shaped_glyphs[shape_from + i].position = ImtPosition {
					x: glyph_x + x_offset,
					y: glyph_y
				};
				
				x += hori_adv;
			}
			
			lines.push((shape_from, shape_from + infos_len, line_max_x));
			break 'line;
		}
		
		// -- Shift Wrapping -- //
		
		if let &ImtTextWrap::Shift = &opts.text_wrap {
			let mut remove_glyphs = Vec::new();
			let mut i_adjust = 0;
			
			for (start, end, width) in &mut lines {
				if start != end {
					let shift = (opts.body_width / (font_props.scaler * opts.text_height)) - *width;
					let mut new_start = None;
					
					for i in *start..*end {
						if new_start.is_none() {
							if imt_shaped_glyphs[i].position.x > shift {
								new_start = Some(i);
								imt_shaped_glyphs[i].position.x -= shift;
							} else {
								remove_glyphs.push(i);
							}
						} else {
							imt_shaped_glyphs[i].position.x -= shift;
						}		
					}
					
					if let Some(new_start) = new_start {
						let start_diff = new_start - *start;
						*start = new_start - i_adjust;
						*end = *end - i_adjust;
						*width -= shift;
						i_adjust += start_diff;
					}
				}
			}
			
			for (i, g) in imt_shaped_glyphs.split_off(0).into_iter().enumerate() {
				if !remove_glyphs.contains(&i) {
					imt_shaped_glyphs.push(g);
				}
			}
		}
		
		// -- Calculate Overflows -- //
		// TODO: Adjust line width?
		
		{
			let mut remove_glyphs = Vec::new();
			let mut i_adjust = 0;
			let body_width_fu = opts.body_width / (opts.text_height * font_props.scaler);
			let body_height_fu = opts.body_height / (opts.text_height * font_props.scaler);
			
			for (start, end, _width) in &mut lines {
				if start != end {
					let mut removed = 0;
					let mut new_start = None;
					let mut len = 0;
					
					for i in *start..*end {
						let glyph = &mut imt_shaped_glyphs[i];
						let mut remove = false;
						let width = glyph.parsed.max_x - glyph.parsed.min_x;
						let min_x = glyph.position.x + glyph.parsed.min_x;
						let max_x = min_x + width;
						
						if max_x > body_width_fu {
							if min_x > body_width_fu {
								remove = true;
								removed += 1;
							} else {
								glyph.x_overflow = max_x - body_width_fu;
							}
						}
						
						let height = glyph.parsed.max_y - glyph.parsed.min_y;
						let bearing_y = font_props.ascender - glyph.parsed.max_y;
						let min_y = glyph.position.y + bearing_y;
						let max_y = min_y + height;
						
						if max_y > body_height_fu {
							if min_y > body_height_fu {
								remove = true;
								removed += 1;
							} else {
								glyph.y_overflow = max_y - body_height_fu;
							}
						}
						
						if remove {
							remove_glyphs.push(i);
						} else {
							if new_start.is_none() {
								new_start = Some(i);
							}
							
							len += 1;
						}
					}
					
					if let Some(new_start) = new_start {
						*start = new_start - i_adjust;
					}
					
					*end = *start + len;
					i_adjust += removed;
				}
			}
			
			for (i, g) in imt_shaped_glyphs.split_off(0).into_iter().enumerate() {
				if !remove_glyphs.contains(&i) {
					imt_shaped_glyphs.push(g);
				}
			}
		}
				
		
		// -- Horizontal Alignment -- //
		
		let hori_align_scaler = match &opts.hori_align {
			&ImtHoriAlign::Left => 0.0,
			&ImtHoriAlign::Right => 1.0,
			&ImtHoriAlign::Center => 0.5,
		};
		
		if hori_align_scaler != 0.0 {
			for (start, end, width) in &lines {			
				if start != end {
					let space_px = opts.body_width - (*width * font_props.scaler * opts.text_height);
					let space_font_units = space_px / (font_props.scaler * opts.text_height);
					
					//println!("{} {} {} {}", space_px, space_font_units, opts.body_width, *width * (font_props.scaler * opts.text_height));
					let shift = space_font_units * hori_align_scaler;
					
					for i in *start..*end {
						imt_shaped_glyphs[i].position.x += shift;
					}
				}
			}
		}
		
		// Remove New Line Characters
		imt_shaped_glyphs.retain(|g| g.parsed.inner.unicodes[0] != '\n');
		
		Ok(imt_shaped_glyphs)
	}
}
