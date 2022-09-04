use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::atomic::{self, AtomicBool};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use allsorts::binary::read::ReadScope;
use allsorts::font::read_cmap_subtable;
use allsorts::gpos::{self, Info};
use allsorts::gsub::{self, GlyphOrigin, RawGlyph};
use allsorts::layout::{new_layout_cache, GDEFTable, LayoutCache, LayoutTable, GPOS, GSUB};
use allsorts::tables::cmap::{Cmap, CmapSubtable};
use allsorts::tables::glyf::{self, CompositeGlyphArgument, GlyfRecord, GlyfTable};
use allsorts::tables::loca::LocaTable;
use allsorts::tables::{HeadTable, HheaTable, HmtxTable, MaxpTable, OpenTypeData, OpenTypeFont};
use allsorts::tag;
use crossbeam::queue::SegQueue;
use crossbeam::sync::{Parker, Unparker};
use parking_lot::{Condvar, Mutex};

use crate::{ImtError, ImtErrorSrc, ImtErrorTy, ImtGeometry, ImtLang, ImtPoint, ImtScript};

struct ParserReqRes<T> {
    cond: Condvar,
    result: Mutex<Option<Result<T, ImtError>>>,
}

impl<T> ParserReqRes<T> {
    fn new() -> Arc<Self> {
        Arc::new(ParserReqRes {
            cond: Condvar::new(),
            result: Mutex::new(None),
        })
    }

    fn get(&self) -> Result<T, ImtError> {
        let mut result = self.result.lock();

        while result.is_none() {
            self.cond.wait(&mut result);
        }

        result.take().unwrap()
    }

    fn set(&self, res: Result<T, ImtError>) {
        *self.result.lock() = Some(res);
        self.cond.notify_one();
    }
}

enum ParserReq {
    FontProps(Arc<ParserReqRes<ImtFontProps>>),
    RetrieveText(
        Arc<ParserReqRes<Vec<Arc<ImtParsedGlyph>>>>,
        String,
        ImtScript,
        ImtLang,
    ),
    RetrieveInfo(
        Arc<ParserReqRes<Vec<Info>>>,
        Vec<RawGlyph<()>>,
        ImtScript,
        ImtLang,
    ),
}

pub struct ImtParser {
    worker: Option<JoinHandle<()>>,
    requests: Arc<SegQueue<ParserReq>>,
    unparker: Unparker,
    dropped: Arc<AtomicBool>,
}

impl ImtParser {
    pub fn new(bytes: Vec<u8>) -> Result<Self, ImtError> {
        let requests_orig = Arc::new(SegQueue::new());
        let requests = requests_orig.clone();
        let result_orig: Arc<ParserReqRes<()>> = ParserReqRes::new();
        let result = result_orig.clone();
        let parker = Parker::new();
        let unparker = parker.unparker().clone();
        let dropped_orig = Arc::new(AtomicBool::new(false));
        let dropped = dropped_orig.clone();

        let worker = Some(thread::spawn(move || {
            let mut parser = match ImtParserNonSend::new(bytes) {
                Ok(ok) => {
                    result.set(Ok(()));
                    ok
                },
                Err(e) => {
                    result.set(Err(e));
                    return;
                },
            };

            loop {
                if dropped.load(atomic::Ordering::SeqCst) {
                    return;
                }

                while let Some(req) = requests.pop() {
                    match req {
                        ParserReq::FontProps(res) => res.set(Ok(parser.font_props())),
                        ParserReq::RetrieveText(res, text, script, lang) => {
                            res.set(parser.retreive_text(text, script, lang));
                        },
                        ParserReq::RetrieveInfo(res, glyphs, script, lang) => {
                            res.set(parser.retreive_info(glyphs, script, lang));
                        },
                    }
                }

                parker.park();
            }
        }));

        result_orig.get()?;

        Ok(ImtParser {
            worker,
            requests: requests_orig,
            unparker,
            dropped: dropped_orig,
        })
    }

    pub fn font_props(&self) -> ImtFontProps {
        let res = ParserReqRes::new();
        self.requests.push(ParserReq::FontProps(res.clone()));
        self.unparker.unpark();
        res.get().unwrap()
    }

    pub fn retreive_text<T: AsRef<str>>(
        &self,
        text: T,
        script: ImtScript,
        lang: ImtLang,
    ) -> Result<Vec<Arc<ImtParsedGlyph>>, ImtError> {
        let res = ParserReqRes::new();
        self.requests.push(ParserReq::RetrieveText(
            res.clone(),
            String::from(text.as_ref()),
            script,
            lang,
        ));
        self.unparker.unpark();
        res.get()
    }

    pub fn retreive_info(
        &self,
        raw_glyphs: Vec<RawGlyph<()>>,
        script: ImtScript,
        lang: ImtLang,
    ) -> Result<Vec<Info>, ImtError> {
        let res = ParserReqRes::new();
        self.requests.push(ParserReq::RetrieveInfo(
            res.clone(),
            raw_glyphs,
            script,
            lang,
        ));
        self.unparker.unpark();
        res.get()
    }
}

impl Drop for ImtParser {
    fn drop(&mut self) {
        self.dropped.store(true, atomic::Ordering::SeqCst);
        self.unparker.unpark();

        if let Some(worker) = self.worker.take() {
            worker.join().unwrap();
        }
    }
}

#[allow(dead_code)]
pub struct ImtParserNonSend {
    bytes: Vec<u8>,
    scope: ReadScope<'static>,
    head: HeadTable,
    maxp: MaxpTable,
    cmap: Cmap<'static>,
    cmap_sub: CmapSubtable<'static>,
    hhea: HheaTable,
    hmtx: HmtxTable<'static>,
    loca: LocaTable<'static>,
    glyf: GlyfTable<'static>,
    gdef_op: Option<GDEFTable>,
    gpos_op: Option<LayoutCache<GPOS>>,
    gsub_op: Option<LayoutCache<GSUB>>,
    font_props: ImtFontProps,
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
    pub hori_adv: f32,
    pub geometry: Vec<ImtGeometry>,
}

impl ImtParserNonSend {
    pub fn new(bytes: Vec<u8>) -> Result<Self, ImtError> {
        let OpenTypeFont {
            scope,
            data,
        } = ReadScope::new(unsafe { &*(bytes.as_ref() as *const _) })
            .read::<OpenTypeFont>()
            .map_err(|e| ImtError::allsorts_parse(ImtErrorSrc::File, e))?;

        let otf = match data {
            OpenTypeData::Single(t) => t,
            _ => {
                return Err(ImtError::src_and_ty(
                    ImtErrorSrc::File,
                    ImtErrorTy::FileUnsupportedFormat,
                ))
            },
        };

        let cmap = otf
            .find_table_record(tag::CMAP)
            .ok_or(ImtError::src_and_ty(
                ImtErrorSrc::Cmap,
                ImtErrorTy::FileMissingTable,
            ))?
            .read_table(&scope)
            .map_err(|e| ImtError::allsorts_parse(ImtErrorSrc::Cmap, e))?
            .read::<Cmap>()
            .map_err(|e| ImtError::allsorts_parse(ImtErrorSrc::Cmap, e))?;

        let cmap_sub = read_cmap_subtable(&cmap)
            .map_err(|e| ImtError::allsorts_parse(ImtErrorSrc::Cmap, e))?
            .ok_or(ImtError::src_and_ty(
                ImtErrorSrc::Cmap,
                ImtErrorTy::FileMissingSubTable,
            ))?;

        let maxp = otf
            .find_table_record(tag::MAXP)
            .ok_or(ImtError::src_and_ty(
                ImtErrorSrc::Maxp,
                ImtErrorTy::FileMissingTable,
            ))?
            .read_table(&scope)
            .map_err(|e| ImtError::allsorts_parse(ImtErrorSrc::Maxp, e))?
            .read::<MaxpTable>()
            .map_err(|e| ImtError::allsorts_parse(ImtErrorSrc::Maxp, e))?;

        let gdef_op = match otf.find_table_record(tag::GDEF) {
            None => None,
            Some(v) => {
                Some(
                    v.read_table(&scope)
                        .map_err(|e| ImtError::allsorts_parse(ImtErrorSrc::GDEF, e))?
                        .read::<GDEFTable>()
                        .map_err(|e| ImtError::allsorts_parse(ImtErrorSrc::GDEF, e))?,
                )
            },
        };

        let gpos_op = match otf.find_table_record(tag::GPOS) {
            None => None,
            Some(v) => {
                Some(
                    v.read_table(&scope)
                        .map_err(|e| ImtError::allsorts_parse(ImtErrorSrc::GPOS, e))?
                        .read::<LayoutTable<GPOS>>()
                        .map_err(|e| ImtError::allsorts_parse(ImtErrorSrc::GPOS, e))?,
                )
                .map(|v| new_layout_cache(v))
            },
        };

        let hhea = otf
            .find_table_record(tag::HHEA)
            .ok_or(ImtError::src_and_ty(
                ImtErrorSrc::Hhea,
                ImtErrorTy::FileMissingTable,
            ))?
            .read_table(&scope)
            .map_err(|e| ImtError::allsorts_parse(ImtErrorSrc::Hhea, e))?
            .read::<HheaTable>()
            .map_err(|e| ImtError::allsorts_parse(ImtErrorSrc::Hhea, e))?;

        let hmtx = otf
            .find_table_record(tag::HMTX)
            .ok_or(ImtError::src_and_ty(
                ImtErrorSrc::Hmtx,
                ImtErrorTy::FileMissingTable,
            ))?
            .read_table(&scope)
            .map_err(|e| ImtError::allsorts_parse(ImtErrorSrc::Hmtx, e))?
            .read_dep::<HmtxTable>((maxp.num_glyphs as usize, hhea.num_h_metrics as usize))
            .map_err(|e| ImtError::allsorts_parse(ImtErrorSrc::Hmtx, e))?;

        let head = otf
            .find_table_record(tag::HEAD)
            .ok_or(ImtError::src_and_ty(
                ImtErrorSrc::Head,
                ImtErrorTy::FileMissingTable,
            ))?
            .read_table(&scope)
            .map_err(|e| ImtError::allsorts_parse(ImtErrorSrc::Head, e))?
            .read::<HeadTable>()
            .map_err(|e| ImtError::allsorts_parse(ImtErrorSrc::Head, e))?;

        let loca = otf
            .find_table_record(tag::LOCA)
            .ok_or(ImtError::src_and_ty(
                ImtErrorSrc::Loca,
                ImtErrorTy::FileMissingTable,
            ))?
            .read_table(&scope)
            .map_err(|e| ImtError::allsorts_parse(ImtErrorSrc::Loca, e))?
            .read_dep::<LocaTable>((maxp.num_glyphs as usize, head.index_to_loc_format))
            .map_err(|e| ImtError::allsorts_parse(ImtErrorSrc::Loca, e))?;

        let glyf = otf
            .find_table_record(tag::GLYF)
            .ok_or(ImtError::src_and_ty(
                ImtErrorSrc::Glyf,
                ImtErrorTy::FileMissingTable,
            ))?
            .read_table(&scope)
            .map_err(|e| ImtError::allsorts_parse(ImtErrorSrc::Glyf, e))?
            .read_dep::<GlyfTable>(unsafe { &*(&loca as *const _) })
            .map_err(|e| ImtError::allsorts_parse(ImtErrorSrc::Glyf, e))?;

        let gsub_op = match otf.find_table_record(tag::GSUB) {
            None => None,
            Some(v) => {
                Some(
                    v.read_table(&scope)
                        .map_err(|e| ImtError::allsorts_parse(ImtErrorSrc::Gsub, e))?
                        .read::<LayoutTable<GSUB>>()
                        .map_err(|e| ImtError::allsorts_parse(ImtErrorSrc::Gsub, e))?,
                )
                .map(|v| new_layout_cache(v))
            },
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

        Ok(ImtParserNonSend {
            parsed_glyphs: BTreeMap::new(),
            bytes,
            scope,
            head,
            maxp,
            cmap,
            cmap_sub: cmap_sub.1,
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

    pub fn retreive_info(
        &mut self,
        raw_glyphs: Vec<RawGlyph<()>>,
        script: ImtScript,
        lang: ImtLang,
    ) -> Result<Vec<Info>, ImtError> {
        let mut infos = Info::init_from_glyphs(self.gdef_op.as_ref(), raw_glyphs);

        if let Some(gpos) = self.gpos_op.take() {
            let gpos_rc = Rc::new(gpos);

            gpos::apply(
                &gpos_rc,
                self.gdef_op.as_ref(),
                true,
                &gsub::Features::Mask(gsub::FeatureMask::default()),
                script.tag(),
                Some(lang.tag()),
                &mut infos,
            )
            .map_err(|e| ImtError::allsorts_parse(ImtErrorSrc::GPOS, e))?;

            self.gpos_op = Some(Rc::try_unwrap(gpos_rc).ok().unwrap());
        }

        Ok(infos)
    }

    fn glyph_for_char(&mut self, c: char) -> Result<RawGlyph<()>, ImtError> {
        let index = self
            .cmap_sub
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
            );

        Ok(RawGlyph {
            unicodes: [c].into(),
            glyph_index: index,
            liga_component_pos: 0,
            glyph_origin: GlyphOrigin::Char(c),
            small_caps: false,
            multi_subst_dup: false,
            is_vert_alt: false,
            fake_bold: false,
            fake_italic: false,
            extra_data: (),
            variation: None,
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
            gsub::apply(
                sub_glyph.glyph_index,
                &gsub,
                self.gdef_op.as_ref(),
                script.tag(),
                Some(lang.tag()),
                &gsub::Features::Mask(gsub::FeatureMask::default()),
                self.maxp.num_glyphs,
                &mut glyphs,
            )
            .map_err(|e| ImtError::allsorts_shaping(ImtErrorSrc::Gsub, e))?;
        }

        let mut imt_raw_glyphs = Vec::new();

        for glyph in glyphs {
            let index = glyph.glyph_index;

            if self.parsed_glyphs.get(&index).is_none() {
                let mut geometry_indexes: Vec<(u16, f32, f32)> = vec![(index, 0.0, 0.0)];
                let mut geometry = Vec::new();
                let mut min_x = None;
                let mut min_y = None;
                let mut max_x = None;
                let mut max_y = None;

                while let Some((geometry_index, gox, goy)) = geometry_indexes.pop() {
                    let glyf_record = self.glyf.records.get_mut(geometry_index as usize).ok_or(
                        ImtError::src_and_ty(ImtErrorSrc::Glyf, ImtErrorTy::MissingGlyph),
                    )?;

                    if let Some(parsed_record) = match &glyf_record {
                        &GlyfRecord::Present {
                            ref scope, ..
                        } => {
                            Some(GlyfRecord::Parsed(scope.read::<glyf::Glyph>().map_err(
                                |e| ImtError::allsorts_parse(ImtErrorSrc::Glyf, e),
                            )?))
                        },
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
                                                            (contour[p_i].1 + contour[j].1) / 2.0,
                                                            (contour[p_i].2 + contour[j].2) / 2.0,
                                                        )
                                                    };

                                                    let c = if simple.flags[contour[n_i].0]
                                                        .is_on_curve()
                                                    {
                                                        (contour[n_i].1, contour[n_i].2)
                                                    } else {
                                                        (
                                                            (contour[n_i].1 + contour[j].1) / 2.0,
                                                            (contour[n_i].2 + contour[j].2) / 2.0,
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

                                                    if simple.flags[contour[n_i].0].is_on_curve() {
                                                        geometry.push(ImtGeometry::Line([
                                                            ImtPoint {
                                                                x: contour[j].1 as f32 + gox as f32,
                                                                y: contour[j].2 as f32 + goy as f32,
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
                                    glyphs, ..
                                } => {
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
                                    }
                                },
                            };
                        },
                        &GlyfRecord::Empty => continue,
                        &GlyfRecord::Present {
                            ..
                        } => panic!("Glyph should already be parsed!"),
                    };
                }

                let hori_adv = self
                    .hmtx
                    .horizontal_advance(index, self.hhea.num_h_metrics)
                    .map_err(|e| ImtError::allsorts_parse(ImtErrorSrc::Glyph, e))?
                    as f32;

                self.parsed_glyphs.insert(
                    index,
                    Arc::new(ImtParsedGlyph {
                        inner: glyph,
                        min_x: min_x.unwrap_or(0.0),
                        min_y: min_y.unwrap_or(0.0),
                        max_x: max_x.unwrap_or(0.0),
                        max_y: max_y.unwrap_or(0.0),
                        hori_adv,
                        geometry,
                    }),
                );
            }

            imt_raw_glyphs.push(self.parsed_glyphs.get(&index).unwrap().clone());
        }

        Ok(imt_raw_glyphs)
    }
}
