use crate::{ImtError, ImtErrorSrc, ImtErrorTy, ImtGeometry, ImtLang, ImtPoint, ImtScript};

use allsorts::{
	binary::read::ReadScope,
	font_data_impl::read_cmap_subtable,
	gsub::{gsub_apply_default, GlyphOrigin, RawGlyph},
	layout::{new_layout_cache, GDEFTable, LayoutCache, LayoutTable, GPOS, GSUB},
	tables::{
		cmap::{Cmap, CmapSubtable},
		glyf::{self, CompositeGlyphArgument, GlyfRecord, GlyfTable},
		loca::LocaTable,
		HeadTable, HheaTable, HmtxTable, MaxpTable, OpenTypeFile, OpenTypeFont,
	},
	tag,
};
use std::{collections::BTreeMap, sync::Arc};

unsafe impl Send for ImtParser {}

// --- WARNING ABOUT SEND IMPLEMENTATION ----------------------------------------
// From my understanding of the allsorts library and how it is used in this
// this library it is safe to force impl Send for this struct. As long as
// the following conditions are met:
// -	Only one thread at time can access fields be it for read or writes.
// Fields which are public are only public to this crate.
// -	All data used from the parser does not contain any references or
// Rc's from the parser. All data created from this parser are owned or
// is thread safe.
// --------------------------------------------------------------------------------

#[allow(dead_code)]
pub struct ImtParser {
	bytes: Vec<u8>,
	scope: ReadScope<'static>,
	pub(crate) head: HeadTable,
	pub(crate) maxp: MaxpTable,
	pub(crate) cmap: Cmap<'static>,
	pub(crate) cmap_sub: CmapSubtable<'static>,
	pub(crate) hhea: HheaTable,
	pub(crate) hmtx: HmtxTable<'static>,
	pub(crate) loca: LocaTable<'static>,
	pub(crate) glyf: GlyfTable<'static>,
	pub(crate) gdef_op: Option<GDEFTable>,
	pub(crate) gpos_op: Option<LayoutCache<GPOS>>,
	pub(crate) gsub_op: Option<LayoutCache<GSUB>>,
	pub(crate) font_props: ImtFontProps,
	parsed_glyphs: BTreeMap<u16, Arc<ImtParsedGlyph>>,
}

#[derive(Debug, Clone)]
pub struct ImtFontProps {
	pub scaler: f32,
	pub ascender: f32,
	pub descender: f32,
	pub line_gap: f32,
}

pub struct ImtParsedGlyph {
	pub inner: RawGlyph<()>,
	pub min_x: f32,
	pub min_y: f32,
	pub max_x: f32,
	pub max_y: f32,
	pub geometry: Vec<ImtGeometry>,
}

impl ImtParser {
	pub fn new(bytes: Vec<u8>) -> Result<Self, ImtError> {
		let OpenTypeFile {
			scope,
			font,
		} = ReadScope::new(unsafe { &*(bytes.as_ref() as *const _) })
			.read::<OpenTypeFile>()
			.map_err(|e| ImtError::allsorts_parse(ImtErrorSrc::File, e))?;

		let otf = match font {
			OpenTypeFont::Single(t) => t,
			OpenTypeFont::Collection(_) =>
				return Err(ImtError::src_and_ty(
					ImtErrorSrc::File,
					ImtErrorTy::FileUnsupportedFormat,
				)),
		};

		let cmap = otf
			.find_table_record(tag::CMAP)
			.ok_or(ImtError::src_and_ty(ImtErrorSrc::Cmap, ImtErrorTy::FileMissingTable))?
			.read_table(&scope)
			.map_err(|e| ImtError::allsorts_parse(ImtErrorSrc::Cmap, e))?
			.read::<Cmap>()
			.map_err(|e| ImtError::allsorts_parse(ImtErrorSrc::Cmap, e))?;

		let cmap_sub = read_cmap_subtable(&cmap)
			.map_err(|e| ImtError::allsorts_parse(ImtErrorSrc::Cmap, e))?
			.ok_or(ImtError::src_and_ty(ImtErrorSrc::Cmap, ImtErrorTy::FileMissingSubTable))?;

		let maxp = otf
			.find_table_record(tag::MAXP)
			.ok_or(ImtError::src_and_ty(ImtErrorSrc::Maxp, ImtErrorTy::FileMissingTable))?
			.read_table(&scope)
			.map_err(|e| ImtError::allsorts_parse(ImtErrorSrc::Maxp, e))?
			.read::<MaxpTable>()
			.map_err(|e| ImtError::allsorts_parse(ImtErrorSrc::Maxp, e))?;

		let gdef_op = match otf.find_table_record(tag::GDEF) {
			None => None,
			Some(v) =>
				Some(
					v.read_table(&scope)
						.map_err(|e| ImtError::allsorts_parse(ImtErrorSrc::GDEF, e))?
						.read::<GDEFTable>()
						.map_err(|e| ImtError::allsorts_parse(ImtErrorSrc::GDEF, e))?,
				),
		};

		let gpos_op = match otf.find_table_record(tag::GPOS) {
			None => None,
			Some(v) =>
				Some(
					v.read_table(&scope)
						.map_err(|e| ImtError::allsorts_parse(ImtErrorSrc::GPOS, e))?
						.read::<LayoutTable<GPOS>>()
						.map_err(|e| ImtError::allsorts_parse(ImtErrorSrc::GPOS, e))?,
				)
				.map(|v| new_layout_cache(v)),
		};

		let hhea = otf
			.find_table_record(tag::HHEA)
			.ok_or(ImtError::src_and_ty(ImtErrorSrc::Hhea, ImtErrorTy::FileMissingTable))?
			.read_table(&scope)
			.map_err(|e| ImtError::allsorts_parse(ImtErrorSrc::Hhea, e))?
			.read::<HheaTable>()
			.map_err(|e| ImtError::allsorts_parse(ImtErrorSrc::Hhea, e))?;

		let hmtx = otf
			.find_table_record(tag::HMTX)
			.ok_or(ImtError::src_and_ty(ImtErrorSrc::Hmtx, ImtErrorTy::FileMissingTable))?
			.read_table(&scope)
			.map_err(|e| ImtError::allsorts_parse(ImtErrorSrc::Hmtx, e))?
			.read_dep::<HmtxTable>((maxp.num_glyphs as usize, hhea.num_h_metrics as usize))
			.map_err(|e| ImtError::allsorts_parse(ImtErrorSrc::Hmtx, e))?;

		let head = otf
			.find_table_record(tag::HEAD)
			.ok_or(ImtError::src_and_ty(ImtErrorSrc::Head, ImtErrorTy::FileMissingTable))?
			.read_table(&scope)
			.map_err(|e| ImtError::allsorts_parse(ImtErrorSrc::Head, e))?
			.read::<HeadTable>()
			.map_err(|e| ImtError::allsorts_parse(ImtErrorSrc::Head, e))?;

		let loca = otf
			.find_table_record(tag::LOCA)
			.ok_or(ImtError::src_and_ty(ImtErrorSrc::Loca, ImtErrorTy::FileMissingTable))?
			.read_table(&scope)
			.map_err(|e| ImtError::allsorts_parse(ImtErrorSrc::Loca, e))?
			.read_dep::<LocaTable>((maxp.num_glyphs as usize, head.index_to_loc_format))
			.map_err(|e| ImtError::allsorts_parse(ImtErrorSrc::Loca, e))?;

		let glyf = otf
			.find_table_record(tag::GLYF)
			.ok_or(ImtError::src_and_ty(ImtErrorSrc::Glyf, ImtErrorTy::FileMissingTable))?
			.read_table(&scope)
			.map_err(|e| ImtError::allsorts_parse(ImtErrorSrc::Glyf, e))?
			.read_dep::<GlyfTable>(unsafe { &*(&loca as *const _) })
			.map_err(|e| ImtError::allsorts_parse(ImtErrorSrc::Glyf, e))?;

		let gsub_op = match otf.find_table_record(tag::GSUB) {
			None => None,
			Some(v) =>
				Some(
					v.read_table(&scope)
						.map_err(|e| ImtError::allsorts_parse(ImtErrorSrc::Gsub, e))?
						.read::<LayoutTable<GSUB>>()
						.map_err(|e| ImtError::allsorts_parse(ImtErrorSrc::Gsub, e))?,
				)
				.map(|v| new_layout_cache(v)),
		};

		let default_dpi = 72.0;
		let default_pixel_height = 1.0;
		// TODO 1.00 should be 1.33 but why?
		let scaler = ((default_pixel_height * 1.00) * default_dpi)
			/ (default_dpi * head.units_per_em as f32);
		let line_gap = hhea.line_gap as f32 + hhea.ascender as f32
			- (hhea.ascender as f32 + head.y_min as f32);

		let font_props = ImtFontProps {
			scaler,
			// TODO: (head.units_per_em as f32 / 22.0).floor()
			// 			This is needed to adjust the y_min for some reason
			ascender: hhea.ascender as f32
				+ head.y_min as f32
				+ (head.units_per_em as f32 / 22.0).floor(),
			descender: hhea.descender as f32,
			line_gap,
		};

		Ok(ImtParser {
			parsed_glyphs: BTreeMap::new(),
			bytes,
			scope,
			head,
			maxp,
			cmap,
			cmap_sub,
			hhea,
			hmtx,
			loca,
			glyf,
			gdef_op,
			gpos_op,
			gsub_op,
			font_props,
		})
	}

	pub fn font_props(&mut self) -> ImtFontProps {
		self.font_props.clone()
	}

	fn glyph_for_char(&mut self, c: char) -> Result<RawGlyph<()>, ImtError> {
		let index = Some(
			self.cmap_sub
				.map_glyph(c as u32)
				.map_err(|e| ImtError::allsorts_parse(ImtErrorSrc::Cmap, e))?
				.unwrap_or(
					self.cmap_sub
						.map_glyph('?' as u32)
						.map_err(|e| ImtError::allsorts_parse(ImtErrorSrc::Cmap, e))?
						.ok_or(ImtError::src_and_ty(
							ImtErrorSrc::Cmap,
							ImtErrorTy::MissingGlyph,
						))?,
				),
		);

		Ok(RawGlyph {
			unicodes: vec![c],
			glyph_index: index,
			liga_component_pos: 0,
			glyph_origin: GlyphOrigin::Char(c),
			small_caps: false,
			multi_subst_dup: false,
			is_vert_alt: false,
			fake_bold: false,
			fake_italic: false,
			extra_data: (),
		})
	}

	pub fn retreive_text<T: AsRef<str>>(
		&mut self,
		text: T,
		script: ImtScript,
		lang: ImtLang,
	) -> Result<Vec<Arc<ImtParsedGlyph>>, ImtError> {
		let mut glyphs = Vec::new();

		for c in text.as_ref().replace("\r", "").chars() {
			glyphs.push(self.glyph_for_char(c)?);
		}

		let sub_glyph = self.glyph_for_char('?')?;

		if let &Some(ref gsub) = &self.gsub_op {
			gsub_apply_default(
				&|| vec![sub_glyph.clone()],
				&gsub,
				self.gdef_op.as_ref(),
				script.tag(),
				lang.tag(),
				false,
				self.maxp.num_glyphs,
				&mut glyphs,
			)
			.map_err(|e| ImtError::allsorts_shaping(ImtErrorSrc::Gsub, e))?;
		}

		let mut imt_raw_glyphs = Vec::new();

		for glyph in glyphs {
			let index = glyph.glyph_index.unwrap();

			if self.parsed_glyphs.get(&index).is_none() {
				let mut geometry_indexes: Vec<(u16, f32, f32)> = vec![(index, 0.0, 0.0)];
				let mut geometry = Vec::new();
				let mut min_x = None;
				let mut min_y = None;
				let mut max_x = None;
				let mut max_y = None;

				while let Some((geometry_index, gox, goy)) = geometry_indexes.pop() {
					let glyf_record =
						self.glyf.records.get_mut(geometry_index as usize).ok_or(
							ImtError::src_and_ty(ImtErrorSrc::Glyf, ImtErrorTy::MissingGlyph),
						)?;

					if let Some(parsed_record) = match &glyf_record {
						&GlyfRecord::Present(ref record_scope) =>
							Some(GlyfRecord::Parsed(
								record_scope.read::<glyf::Glyph>().map_err(|e| {
									ImtError::allsorts_parse(ImtErrorSrc::Glyf, e)
								})?,
							)),
						_ => None,
					} {
						*glyf_record = parsed_record;
					}

					match &glyf_record {
						&GlyfRecord::Parsed(ref glfy_glyph) => {
							let g_min_x = glfy_glyph.bounding_box.x_min as f32 - gox as f32;
							let g_min_y = glfy_glyph.bounding_box.y_min as f32 - goy as f32;
							let g_max_x = glfy_glyph.bounding_box.x_max as f32 - gox as f32;
							let g_max_y = glfy_glyph.bounding_box.y_max as f32 - goy as f32;

							if min_x.is_none() || g_min_x < *min_x.as_ref().unwrap() {
								min_x = Some(g_min_x);
							}

							if min_y.is_none() || g_min_y < *min_y.as_ref().unwrap() {
								min_y = Some(g_min_y);
							}

							if max_x.is_none() || g_max_x > *max_x.as_ref().unwrap() {
								max_x = Some(g_max_x);
							}

							if max_y.is_none() || g_max_y > *max_y.as_ref().unwrap() {
								max_y = Some(g_max_y);
							}

							match &glfy_glyph.data {
								&glyf::GlyphData::Simple(ref simple) => {
									let mut contour = Vec::new();

									for i in 0..simple.coordinates.len() {
										contour.push((
											i,
											simple.coordinates[i].0 as f32,
											simple.coordinates[i].1 as f32,
										));

										if simple.end_pts_of_contours.contains(&(i as u16)) {
											for j in 0..contour.len() {
												if !simple.flags[contour[j].0].is_on_curve() {
													let p_i = if j == 0 {
														contour.len() - 1
													} else {
														j - 1
													};
													let n_i = if j == contour.len() - 1 {
														0
													} else {
														j + 1
													};

													let a = if simple.flags[contour[p_i].0]
														.is_on_curve()
													{
														(contour[p_i].1, contour[p_i].2)
													} else {
														(
															(contour[p_i].1 + contour[j].1)
																/ 2.0,
															(contour[p_i].2 + contour[j].2)
																/ 2.0,
														)
													};

													let c = if simple.flags[contour[n_i].0]
														.is_on_curve()
													{
														(contour[n_i].1, contour[n_i].2)
													} else {
														(
															(contour[n_i].1 + contour[j].1)
																/ 2.0,
															(contour[n_i].2 + contour[j].2)
																/ 2.0,
														)
													};

													let b = (contour[j].1, contour[j].2);

													geometry.push(ImtGeometry::Curve([
														ImtPoint {
															x: a.0 as f32 + gox as f32,
															y: a.1 as f32 + goy as f32,
														},
														ImtPoint {
															x: b.0 as f32 + gox as f32,
															y: b.1 as f32 + goy as f32,
														},
														ImtPoint {
															x: c.0 as f32 + gox as f32,
															y: c.1 as f32 + goy as f32,
														},
													]));
												} else {
													let n_i = if j == contour.len() - 1 {
														0
													} else {
														j + 1
													};

													if simple.flags[contour[n_i].0]
														.is_on_curve()
													{
														geometry.push(ImtGeometry::Line([
															ImtPoint {
																x: contour[j].1 as f32
																	+ gox as f32,
																y: contour[j].2 as f32
																	+ goy as f32,
															},
															ImtPoint {
																x: contour[n_i].1 as f32
																	+ gox as f32,
																y: contour[n_i].2 as f32
																	+ goy as f32,
															},
														]));
													}
												}
											}

											contour.clear();
										}
									}
								},
								glyf::GlyphData::Composite {
									glyphs,
									..
								} =>
									for glyph in glyphs {
										let x: f32 = match glyph.argument1 {
											CompositeGlyphArgument::U8(v) => v as f32,
											CompositeGlyphArgument::I8(v) => v as f32,
											CompositeGlyphArgument::U16(v) => v as f32,
											CompositeGlyphArgument::I16(v) => v as f32,
										};

										let y: f32 = match glyph.argument2 {
											CompositeGlyphArgument::U8(v) => v as f32,
											CompositeGlyphArgument::I8(v) => v as f32,
											CompositeGlyphArgument::U16(v) => v as f32,
											CompositeGlyphArgument::I16(v) => v as f32,
										};

										geometry_indexes.push((glyph.glyph_index, x, y));
									},
							};
						},
						&GlyfRecord::Empty => continue,
						&GlyfRecord::Present(_) => panic!("Glyph should already be parsed!"),
					};
				}

				self.parsed_glyphs.insert(
					index,
					Arc::new(ImtParsedGlyph {
						inner: glyph,
						min_x: min_x.unwrap_or(0.0),
						min_y: min_y.unwrap_or(0.0),
						max_x: max_x.unwrap_or(0.0),
						max_y: max_y.unwrap_or(0.0),
						geometry,
					}),
				);
			}

			imt_raw_glyphs.push(self.parsed_glyphs.get(&index).unwrap().clone());
		}

		Ok(imt_raw_glyphs)
	}
}
