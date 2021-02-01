use crate::{shaders::glyph_cs, ImtError, ImtGlyphBitmap, ImtParser, ImtShapedGlyph};
use crossbeam::sync::{Parker, Unparker};
use ordered_float::OrderedFloat;
use parking_lot::Mutex;
use std::{collections::BTreeMap, iter, sync::Arc};
use vulkano::{
	buffer::{cpu_access::CpuAccessibleBuffer, device_local::DeviceLocalBuffer, BufferUsage},
	command_buffer::{AutoCommandBufferBuilder, CommandBuffer},
	device::{Device, Queue},
	sync::GpuFuture,
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
pub struct ImtRasterOpts {
	pub fill_quality: ImtFillQuality,
	pub sample_quality: ImtSampleQuality,
	pub align_whole_pixels: bool,
}

impl Default for ImtRasterOpts {
	fn default() -> Self {
		ImtRasterOpts {
			fill_quality: ImtFillQuality::Normal,
			sample_quality: ImtSampleQuality::Normal,
			align_whole_pixels: true,
		}
	}
}

pub struct ImtRasteredGlyph {
	pub shaped: ImtShapedGlyph,
	pub bitmap: Arc<ImtGlyphBitmap>,
}

#[derive(Clone)]
enum RasterCacheState {
	Completed(Arc<ImtGlyphBitmap>),
	Incomplete(Vec<Unparker>),
	Errored(ImtError),
}

#[allow(dead_code)]
pub struct ImtRaster {
	opts: ImtRasterOpts,
	device: Arc<Device>,
	queue: Arc<Queue>,
	glyph_cs: glyph_cs::Shader,
	sample_data_buf: Arc<DeviceLocalBuffer<[[f32; 4]]>>,
	ray_data_buf: Arc<DeviceLocalBuffer<[[f32; 4]]>>,
	sample_count: usize,
	ray_count: usize,
	cache: Mutex<BTreeMap<(OrderedFloat<f32>, u16), RasterCacheState>>,
}

impl ImtRaster {
	pub fn new(
		device: Arc<Device>,
		queue: Arc<Queue>,
		opts: ImtRasterOpts,
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

		let sample_data_cpu_buf: Arc<CpuAccessibleBuffer<[[f32; 4]]>> =
			CpuAccessibleBuffer::from_iter(
				device.clone(),
				BufferUsage {
					storage_buffer: true,
					transfer_source: true,
					..BufferUsage::none()
				},
				false,
				sample_data.into_iter(),
			)
			.unwrap();

		let ray_count = match &opts.fill_quality {
			&ImtFillQuality::Fast => 3,
			&ImtFillQuality::Normal => 5,
			&ImtFillQuality::Best => 13,
		};

		let mut ray_data: Vec<[f32; 4]> = Vec::with_capacity(ray_count);

		for i in 0..ray_count {
			let rad = (i as f32 * (360.0 / ray_count as f32)).to_radians();
			ray_data.push([rad.cos(), rad.sin(), 0.0, 0.0]);
		}

		let ray_data_cpu_buf: Arc<CpuAccessibleBuffer<[[f32; 4]]>> =
			CpuAccessibleBuffer::from_iter(
				device.clone(),
				BufferUsage {
					storage_buffer: true,
					transfer_source: true,
					..BufferUsage::none()
				},
				false,
				ray_data.into_iter(),
			)
			.unwrap();

		let sample_data_dev_buf: Arc<DeviceLocalBuffer<[[f32; 4]]>> = DeviceLocalBuffer::array(
			device.clone(),
			sample_count,
			BufferUsage {
				storage_buffer: true,
				transfer_destination: true,
				uniform_buffer: true,
				..BufferUsage::none()
			},
			iter::once(queue.family()),
		)
		.unwrap();

		let ray_data_dev_buf: Arc<DeviceLocalBuffer<[[f32; 4]]>> = DeviceLocalBuffer::array(
			device.clone(),
			ray_count,
			BufferUsage {
				storage_buffer: true,
				transfer_destination: true,
				uniform_buffer: true,
				..BufferUsage::none()
			},
			iter::once(queue.family()),
		)
		.unwrap();

		let mut cmd_buf =
			AutoCommandBufferBuilder::new(device.clone(), queue.family()).unwrap();

		cmd_buf
			.copy_buffer(sample_data_cpu_buf.clone(), sample_data_dev_buf.clone())
			.unwrap()
			.copy_buffer(ray_data_cpu_buf.clone(), ray_data_dev_buf.clone())
			.unwrap();

		cmd_buf
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
			cache: Mutex::new(BTreeMap::new()),
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

	#[allow(unused_assignments)]
	pub fn raster_shaped_glyphs(
		&self,
		parser: &ImtParser,
		text_height: f32,
		shaped_glyphs: Vec<ImtShapedGlyph>,
	) -> Result<Vec<ImtRasteredGlyph>, ImtError> {
		let mut rastered_glyphs_out = Vec::new();
		let mut cache_lk_op = None;
		let height_key = OrderedFloat::from(text_height);

		'glyphs: for shaped in shaped_glyphs {
			let index = shaped.parsed.inner.glyph_index;

			// Acquire a lock to the cache if it isn't already present
			if cache_lk_op.is_none() {
				cache_lk_op = Some(self.cache.lock());
			}

			let mut parker_op = None;

			// Obtain the current cache state
			if let Some(cache_state) =
				cache_lk_op.as_mut().unwrap().get_mut(&(height_key, index))
			{
				match cache_state {
					// This glyph has already be completed!
					&mut RasterCacheState::Completed(ref bitmap) => {
						rastered_glyphs_out.push(ImtRasteredGlyph {
							shaped,
							bitmap: bitmap.clone(),
						});

						continue;
					},
					// This glyph is currently in the progress of be rasterized. Add this
					// thread's unparker so we can wait for it to complete.
					&mut RasterCacheState::Incomplete(ref mut unparkers) => {
						let parker = Parker::new();
						unparkers.push(parker.unparker().clone());
						parker_op = Some(parker);
					},
					// The last attempted seem'd to have error, try again why not.
					&mut RasterCacheState::Errored(_) => (),
				}
			}

			// Another thread is in the progress of rasterizing, so park.
			if let Some(parker) = parker_op {
				// Loop as the parker my spuriously wake up!
				loop {
					// Drop the lock to not hold things up.
					cache_lk_op = None;
					parker.park();

					// Reobtain the lock
					cache_lk_op = Some(self.cache.lock());

					// Should be safe to unwrap as the state should already be present given
					// the previous logic.
					let cache_state =
						cache_lk_op.as_ref().unwrap().get(&(height_key, index)).unwrap();

					match cache_state {
						// As expected the glyph is completed.
						&RasterCacheState::Completed(ref bitmap) => {
							rastered_glyphs_out.push(ImtRasteredGlyph {
								shaped,
								bitmap: bitmap.clone(),
							});

							continue 'glyphs;
						},
						// Seems this thread has spuriously woken up, go back to sleep.
						&RasterCacheState::Incomplete(_) => continue,
						// The last attempted seem'd to have error, try again why not.
						&RasterCacheState::Errored(_) => break,
					}
				}
			}

			// Made it here, so assume that the glyph needs to be rasterized yet.

			// The cache lock should still be held, but check.
			if cache_lk_op.is_none() {
				cache_lk_op = Some(self.cache.lock());
			}

			// Update the cache to inform it that this thread is going to rasterize the glyph.
			cache_lk_op
				.as_mut()
				.unwrap()
				.insert((height_key, index), RasterCacheState::Incomplete(Vec::new()));

			// Drop the lock so other threads can keep doing things.
			cache_lk_op = None;

			let mut bitmap =
				ImtGlyphBitmap::new(parser, shaped.parsed.clone(), text_height, &self.opts);
			bitmap.create_outline();

			if let Err(e) = bitmap.raster(self) {
				// Seems we have errored, up the cache and inform other threads.
				// Reobtain the lock
				cache_lk_op = Some(self.cache.lock());

				// Update the state to errored and retrieve the old one.
				let old_state = cache_lk_op
					.as_mut()
					.unwrap()
					.insert((height_key, index), RasterCacheState::Errored(e.clone()));

				// Inform all the other threads that may have been waiting.
				if let Some(RasterCacheState::Incomplete(unparkers)) = old_state {
					for unparker in unparkers {
						unparker.unpark();
					}
				}

				// Finally return the error
				return Err(e);
			}

			// The glyph seems to have rastered sucessfully!

			// Wrap the bitmap into its final form.
			let bitmap = Arc::new(bitmap);

			// Reobtain the lock
			cache_lk_op = Some(self.cache.lock());

			// Update the state to completed and retrieve the old one.
			let old_state = cache_lk_op
				.as_mut()
				.unwrap()
				.insert((height_key, index), RasterCacheState::Completed(bitmap.clone()));

			// Inform all the other threads that may have been waiting.
			if let Some(RasterCacheState::Incomplete(unparkers)) = old_state {
				for unparker in unparkers {
					unparker.unpark();
				}
			}

			rastered_glyphs_out.push(ImtRasteredGlyph {
				shaped,
				bitmap: bitmap.clone(),
			});
		}

		Ok(rastered_glyphs_out)
	}
}
