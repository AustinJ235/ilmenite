use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;

use vulkano::device::{Device, Queue};

use crate::{
    ImtError, ImtErrorSrc, ImtErrorTy, ImtGlyph, ImtLang, ImtParser, ImtRaster, ImtRasterOpts,
    ImtScript, ImtShapeOpts, ImtShaper,
};

#[derive(Debug, Clone, Copy, PartialEq, Hash, Eq)]
pub enum ImtWeight {
    Thin,
    ExtraLight,
    Light,
    Normal,
    Medium,
    SemiBold,
    Bold,
    ExtraBold,
    UltraBold,
}

#[derive(Debug, Clone, PartialEq, Hash, Eq)]
pub(crate) struct ImtFontKey {
    pub family: String,
    pub weight: ImtWeight,
}

pub struct ImtFont {
    family: String,
    weight: ImtWeight,
    parser: ImtParser,
    shaper: ImtShaper,
    raster: ImtRaster,
}

impl ImtFont {
    pub fn from_file_gpu<F: Into<String>, P: AsRef<Path>>(
        family: F,
        weight: ImtWeight,
        raster_ops: ImtRasterOpts,
        device: Arc<Device>,
        queue: Arc<Queue>,
        path: P,
    ) -> Result<ImtFont, ImtError> {
        let mut handle = File::open(path.as_ref())
            .map_err(|_| ImtError::src_and_ty(ImtErrorSrc::File, ImtErrorTy::FileRead))?;
        let mut bytes = Vec::new();
        handle
            .read_to_end(&mut bytes)
            .map_err(|_| ImtError::src_and_ty(ImtErrorSrc::File, ImtErrorTy::FileRead))?;
        Self::from_bytes_gpu(family, weight, raster_ops, device, queue, bytes)
    }

    pub fn from_file_cpu<F: Into<String>, P: AsRef<Path>>(
        family: F,
        weight: ImtWeight,
        raster_ops: ImtRasterOpts,
        path: P,
    ) -> Result<ImtFont, ImtError> {
        let mut handle = File::open(path.as_ref())
            .map_err(|_| ImtError::src_and_ty(ImtErrorSrc::File, ImtErrorTy::FileRead))?;
        let mut bytes = Vec::new();
        handle
            .read_to_end(&mut bytes)
            .map_err(|_| ImtError::src_and_ty(ImtErrorSrc::File, ImtErrorTy::FileRead))?;
        Self::from_bytes_cpu(family, weight, raster_ops, bytes)
    }

    pub fn from_bytes_gpu<F: Into<String>>(
        family: F,
        weight: ImtWeight,
        raster_ops: ImtRasterOpts,
        device: Arc<Device>,
        queue: Arc<Queue>,
        bytes: Vec<u8>,
    ) -> Result<ImtFont, ImtError> {
        let parser = ImtParser::new(bytes)?;
        let shaper = ImtShaper::new()?;
        let raster = ImtRaster::new_gpu(device, queue, raster_ops)?;

        Ok(ImtFont {
            family: family.into(),
            weight,
            parser,
            shaper,
            raster,
        })
    }

    pub fn from_bytes_cpu<F: Into<String>>(
        family: F,
        weight: ImtWeight,
        raster_ops: ImtRasterOpts,
        bytes: Vec<u8>,
    ) -> Result<ImtFont, ImtError> {
        let parser = ImtParser::new(bytes)?;
        let shaper = ImtShaper::new()?;
        let raster = ImtRaster::new_cpu(raster_ops)?;

        Ok(ImtFont {
            family: family.into(),
            weight,
            parser,
            shaper,
            raster,
        })
    }

    pub(crate) fn key(&self) -> ImtFontKey {
        ImtFontKey {
            family: self.family.clone(),
            weight: self.weight.clone(),
        }
    }

    pub fn glyphs_for_text<T: AsRef<str>>(
        &self,
        text_height: f32,
        shape_ops: ImtShapeOpts,
        text: T,
    ) -> Result<Vec<ImtGlyph>, ImtError> {
        // TODO: Auto detect script/lang or require params to specify?
        let script = ImtScript::Default;
        let lang = ImtLang::Default;
        let parsed_glyphs = self.parser.retrieve_text(text, script, lang)?;
        let shaped_glyphs = self.shaper.shape_parsed_glyphs(
            &self.parser,
            script,
            lang,
            shape_ops,
            parsed_glyphs,
        )?;
        let rastered_glyphs =
            self.raster
                .raster_shaped_glyphs(&self.parser, text_height, shaped_glyphs)?;
        let font_props = self.parser.font_props();

        Ok(rastered_glyphs
            .into_iter()
            .map(|g| {
                let bitmap_metrics = g.bitmap.metrics();

                ImtGlyph {
                    x: (g.shaped.position.x * font_props.scaler * text_height)
                        + bitmap_metrics.bearing_x,
                    y: (g.shaped.position.y * font_props.scaler * text_height)
                        + bitmap_metrics.bearing_y,
                    w: bitmap_metrics.width,
                    h: bitmap_metrics.height,
                    crop_x: g.shaped.x_overflow * font_props.scaler * text_height,
                    crop_y: g.shaped.y_overflow * font_props.scaler * text_height,
                    family: self.family.clone(),
                    weight: self.weight.clone(),
                    index: g.shaped.parsed.inner.glyph_index,
                    bitmap: g.bitmap.data(),
                }
            })
            .collect())
    }
}
