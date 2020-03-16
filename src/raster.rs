use crate::{
	shaders::glyph_cs,
	ImtError, ImtGlyphBitmap, ImtParser, ImtShapedGlyph,
};

use ordered_float::OrderedFloat;
use std::{collections::BTreeMap, sync::Arc};
use vulkano::{
	buffer::{cpu_access::CpuAccessibleBuffer, BufferUsage},
	command_buffer::{AutoCommandBufferBuilder, CommandBuffer},
	device::{Device, Queue},
	sync::GpuFuture,
};
use vulkano::buffer::device_local::DeviceLocalBuffer;
use std::iter;

use parking_lot::Mutex;

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
	pub(crate) glyph_cs: glyph_cs::Shader,
	pub(crate) sample_data_buf: Arc<DeviceLocalBuffer<[[f32; 4]]>>,
	pub(crate) ray_data_buf: Arc<DeviceLocalBuffer<[[f32; 4]]>>,
	rastered_glyphs: Mutex<BTreeMap<OrderedFloat<f32>, BTreeMap<u16, Arc<ImtGlyphBitmap>>>>,
	sample_count: usize,
	ray_count: usize,
}

impl ImtRaster {
	pub fn new(
		device: Arc<Device>,
		queue: Arc<Queue>,
		opts: ImtRasterOps,
	) -> Result<Self, ImtError> {
		let glyph_cs = glyph_cs::Shader::load(device.clone()).unwrap();
		let sample_count = match &opts.sample_quality {
			ImtSampleQuality::Fast => 9,
			ImtSampleQuality::Normal => 16,
			ImtSampleQuality::Best => 25,
		};
		
		let mut sample_data: Vec<[f32; 4]> = Vec::with_capacity(sample_count);
		let w = (sample_count as f32).sqrt() as usize;

		for x in 1..=w {
			for y in 1..=w {
				sample_data.push([
					((x as f32 / (w as f32 + 1.0)) * 2.0) - 1.0,
					((y as f32 / (w as f32 + 1.0)) * 2.0) - 1.0,
					0.0,
					0.0,
				]);
			}
		}
		
		let sample_data_cpu_buf: Arc<CpuAccessibleBuffer<[[f32; 4]]>> = CpuAccessibleBuffer::from_iter(
			device.clone(),
			BufferUsage {
				storage_buffer: true,
				transfer_source: true,
				.. BufferUsage::none()
			},
			false,
			sample_data.into_iter()
		).unwrap();
		
		let ray_count = match &opts.fill_quality {
			&ImtFillQuality::Fast => 5,
			&ImtFillQuality::Normal => 9,
			&ImtFillQuality::Best => 13,
		};
		
		let mut ray_data: Vec<[f32; 4]> = Vec::with_capacity(ray_count);

		for i in 0..ray_count {
			let rad = (i as f32 * (360.0 / ray_count as f32)).to_radians();
			ray_data.push([rad.cos(), rad.sin(), 0.0, 0.0]);
		}
		
		let ray_data_cpu_buf: Arc<CpuAccessibleBuffer<[[f32; 4]]>> = CpuAccessibleBuffer::from_iter(
			device.clone(),
			BufferUsage {
				storage_buffer: true,
				transfer_source: true,
				.. BufferUsage::none()
			},
			false,
			ray_data.into_iter()
		).unwrap();
		
		let sample_data_dev_buf: Arc<DeviceLocalBuffer<[[f32; 4]]>> = DeviceLocalBuffer::array(
			device.clone(),
			sample_count,
			BufferUsage {
				storage_buffer: true,
				transfer_destination: true,
				uniform_buffer: true,
				.. BufferUsage::none()
			},
			iter::once(queue.family())
		).unwrap();
		
		let ray_data_dev_buf: Arc<DeviceLocalBuffer<[[f32; 4]]>> = DeviceLocalBuffer::array(
			device.clone(),
			ray_count,
			BufferUsage {
				storage_buffer: true,
				transfer_destination: true,
				uniform_buffer: true,
				.. BufferUsage::none()
			},
			iter::once(queue.family())
		).unwrap();
		
		AutoCommandBufferBuilder::new(
			device.clone(),
			queue.family(),
		).unwrap()
			.copy_buffer(sample_data_cpu_buf.clone(), sample_data_dev_buf.clone())
			.unwrap()
			.copy_buffer(ray_data_cpu_buf.clone(), ray_data_dev_buf.clone())
			.unwrap()
			.build()
			.unwrap()
			.execute(queue.clone())
			.unwrap()
			.then_signal_fence_and_flush()
			.unwrap()
			.wait(None)
			.unwrap();
		
		Ok(ImtRaster {
			device,
			queue,
			opts,
			glyph_cs,
			sample_count,
			ray_count,
			sample_data_buf: sample_data_dev_buf,
			ray_data_buf: ray_data_dev_buf,
			rastered_glyphs: Mutex::new(BTreeMap::new()),
		})
	}
	
	pub fn sample_count(&self) -> usize {
		self.sample_count
	}
	
	pub fn ray_count(&self) -> usize {
		self.ray_count
	}
	
	pub fn device(&self) -> Arc<Device> {
		self.device.clone()
	}
	
	pub fn device_ref(&self) -> &Arc<Device> {
		&self.device
	}
	
	pub fn queue(&self) -> Arc<Queue> {
		self.queue.clone()
	}
	
	pub fn queue_ref(&self) -> &Arc<Queue> {
		&self.queue
	}
	
	pub fn glyph_shader(&self) -> &glyph_cs::Shader {
		&self.glyph_cs
	}
	
	pub fn sample_data_buf(&self) -> Arc<DeviceLocalBuffer<[[f32; 4]]>> {
		self.sample_data_buf.clone()
	}
	
	pub fn ray_data_buf(&self) -> Arc<DeviceLocalBuffer<[[f32; 4]]>> {
		self.ray_data_buf.clone()
	}

	pub fn raster_shaped_glyphs(
		&self,
		parser: &ImtParser,
		text_height: f32,
		shaped_glyphs: Vec<ImtShapedGlyph>,
	) -> Result<Vec<ImtRasteredGlyph>, ImtError> {
		let mut rastered_glyphs = self.rastered_glyphs.lock();
		let bitmap_cache = rastered_glyphs
			.entry(OrderedFloat::from(text_height))
			.or_insert_with(|| BTreeMap::new());
		let mut rastered_glyphs_out = Vec::new();

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

			rastered_glyphs_out.push(ImtRasteredGlyph {
				shaped,
				bitmap,
			});
		}

		Ok(rastered_glyphs_out)
	}
}
