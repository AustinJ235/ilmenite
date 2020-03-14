use crate::{
	shaders::{glyph_base_fs, glyph_post_fs, square_vs},
	ImtError, ImtGlyphBitmap, ImtParser, ImtShaderVert, ImtShapedGlyph,
};

use ordered_float::OrderedFloat;
use std::{collections::BTreeMap, sync::Arc};
use vulkano::{
	buffer::{cpu_access::CpuAccessibleBuffer, BufferUsage},
	device::{Device, Queue},
	sampler::Sampler,
};

#[derive(Clone, Debug, PartialEq)]
pub enum ImtFillQuality {
	Fast,
	Normal,
	Best,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ImtSampleQuality {
	Fast,
	Normal,
	Best,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ImtRasterOps {
	pub fill_quality: ImtFillQuality,
	pub sample_quality: ImtSampleQuality,
}

impl Default for ImtRasterOps {
	fn default() -> Self {
		ImtRasterOps {
			fill_quality: ImtFillQuality::Fast,
			sample_quality: ImtSampleQuality::Fast,
		}
	}
}

pub struct ImtRasteredGlyph {
	pub shaped: ImtShapedGlyph,
	pub bitmap: Arc<ImtGlyphBitmap>,
}

#[allow(dead_code)]
pub struct ImtRaster {
	pub(crate) opts: ImtRasterOps,
	pub(crate) device: Arc<Device>,
	pub(crate) queue: Arc<Queue>,
	pub(crate) square_vs: square_vs::Shader,
	pub(crate) glyph_base_fs: glyph_base_fs::Shader,
	pub(crate) glyph_post_fs: glyph_post_fs::Shader,
	pub(crate) square_buf: Arc<CpuAccessibleBuffer<[ImtShaderVert]>>,
	pub(crate) sample_data_buf: Arc<CpuAccessibleBuffer<glyph_base_fs::ty::SampleData>>,
	pub(crate) ray_data_buf: Arc<CpuAccessibleBuffer<glyph_base_fs::ty::RayData>>,
	pub(crate) sampler: Arc<Sampler>,
	rastered_glyphs: BTreeMap<OrderedFloat<f32>, BTreeMap<u16, Arc<ImtGlyphBitmap>>>,
}

impl ImtRaster {
	pub fn new(
		device: Arc<Device>,
		queue: Arc<Queue>,
		opts: ImtRasterOps,
	) -> Result<Self, ImtError> {
		let square_vs = square_vs::Shader::load(device.clone()).unwrap();
		let glyph_base_fs = glyph_base_fs::Shader::load(device.clone()).unwrap();
		let glyph_post_fs = glyph_post_fs::Shader::load(device.clone()).unwrap();

		// TODO: Use DeviceLocalBuffer
		let square_buf = CpuAccessibleBuffer::from_iter(
			device.clone(),
			BufferUsage {
				vertex_buffer: true,
				..BufferUsage::none()
			},
			false,
			[
				ImtShaderVert {
					position: [-1.0, -1.0],
				},
				ImtShaderVert {
					position: [1.0, -1.0],
				},
				ImtShaderVert {
					position: [1.0, 1.0],
				},
				ImtShaderVert {
					position: [1.0, 1.0],
				},
				ImtShaderVert {
					position: [-1.0, 1.0],
				},
				ImtShaderVert {
					position: [-1.0, -1.0],
				},
			]
			.iter()
			.cloned(),
		)
		.unwrap();

		let mut sample_data = glyph_base_fs::ty::SampleData {
			offsets: [[0.0; 4]; 16],
			samples: 16,
		};

		let w = (sample_data.samples as f32).sqrt() as usize;
		let mut i = 0_usize;

		for x in 1..=w {
			for y in 1..=w {
				sample_data.offsets[i] = [
					((x as f32 / (w as f32 + 1.0)) * 2.0) - 1.0,
					((y as f32 / (w as f32 + 1.0)) * 2.0) - 1.0,
					0.0,
					0.0,
				];
				i += 1;
			}
		}

		let sample_data_buf = CpuAccessibleBuffer::from_data(
			device.clone(),
			BufferUsage {
				uniform_buffer: true,
				..BufferUsage::none()
			},
			false,
			sample_data,
		)
		.unwrap();

		let mut ray_data = glyph_base_fs::ty::RayData {
			dir: [[0.0; 4]; 5],
			count: 5,
		};

		for i in 0..ray_data.dir.len() {
			let rad = (i as f32 * (360.0 / ray_data.dir.len() as f32)).to_radians();
			ray_data.dir[i] = [rad.cos(), rad.sin(), 0.0, 0.0];
		}

		let ray_data_buf = CpuAccessibleBuffer::from_data(
			device.clone(),
			BufferUsage {
				uniform_buffer: true,
				..BufferUsage::none()
			},
			false,
			ray_data,
		)
		.unwrap();

		let sampler = Sampler::new(
			device.clone(),
			vulkano::sampler::Filter::Nearest,
			vulkano::sampler::Filter::Nearest,
			vulkano::sampler::MipmapMode::Nearest,
			vulkano::sampler::SamplerAddressMode::ClampToBorder(
				vulkano::sampler::BorderColor::IntTransparentBlack,
			),
			vulkano::sampler::SamplerAddressMode::ClampToBorder(
				vulkano::sampler::BorderColor::IntTransparentBlack,
			),
			vulkano::sampler::SamplerAddressMode::ClampToBorder(
				vulkano::sampler::BorderColor::IntTransparentBlack,
			),
			0.0,
			1.0,
			0.0,
			1000.0,
		)
		.unwrap();

		Ok(ImtRaster {
			device,
			queue,
			opts,
			square_vs,
			glyph_base_fs,
			glyph_post_fs,
			square_buf,
			sampler,
			sample_data_buf,
			ray_data_buf,
			rastered_glyphs: BTreeMap::new(),
		})
	}

	pub fn raster_shaped_glyphs(
		&mut self,
		parser: &mut ImtParser,
		text_height: f32,
		shaped_glyphs: Vec<ImtShapedGlyph>,
	) -> Result<Vec<ImtRasteredGlyph>, ImtError> {
		let bitmap_cache = unsafe { &mut *(self as *mut Self) }
			.rastered_glyphs
			.entry(OrderedFloat::from(text_height))
			.or_insert_with(|| BTreeMap::new());
		let mut rastered_glyphs = Vec::new();

		for shaped in shaped_glyphs {
			let index = shaped.parsed.inner.glyph_index.unwrap();

			if bitmap_cache.get(&index).is_none() {
				let mut bitmap =
					ImtGlyphBitmap::new(parser, shaped.parsed.clone(), text_height);

				bitmap.create_outline();
				bitmap.raster(self)?;
				bitmap_cache.insert(index, Arc::new(bitmap));
			}

			let bitmap = bitmap_cache.get(&index).unwrap().clone();

			rastered_glyphs.push(ImtRasteredGlyph {
				shaped,
				bitmap,
			});
		}

		Ok(rastered_glyphs)
	}
}
