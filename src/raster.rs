use std::collections::BTreeMap;
use std::iter;
use std::sync::Arc;

use crossbeam::sync::{Parker, Unparker};
use ordered_float::OrderedFloat;
use parking_lot::Mutex;
use vulkano::buffer::cpu_access::CpuAccessibleBuffer;
use vulkano::buffer::device_local::DeviceLocalBuffer;
use vulkano::buffer::BufferUsage;
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, CommandBufferUsage, CopyBufferInfo, PrimaryCommandBuffer,
};
use vulkano::descriptor_set::SingleLayoutDescSetPool;
use vulkano::device::{Device, Queue};
use vulkano::format::Format;
use vulkano::pipeline::{ComputePipeline, Pipeline};
use vulkano::shader::ShaderModule;
use vulkano::sync::GpuFuture;

use crate::shaders::glyph_cs;
use crate::{ImtError, ImtGlyphBitmap, ImtParser, ImtShapedGlyph};

#[derive(Clone, Debug, PartialEq)]
pub enum ImtFillQuality {
    Fast,
    Normal,
    Best,
}

impl ImtFillQuality {
    pub fn ray_count(&self) -> usize {
        match self {
            Self::Fast => 3,
            Self::Normal => 5,
            Self::Best => 13,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ImtSampleQuality {
    Fastest,
    Faster,
    Fast,
    Normal,
    Best,
}

impl ImtSampleQuality {
    pub fn sample_count(&self) -> usize {
        match self {
            Self::Fastest => 1,
            Self::Faster => 4,
            Self::Fast => 9,
            Self::Normal => 16,
            Self::Best => 25,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ImtRasterOpts {
    /// This effects how many rays are casted
    pub fill_quality: ImtFillQuality,
    /// This effects how many samples will be used per subpixel
    pub sample_quality: ImtSampleQuality,
    /// Whether to align bitmaps to whole pixels. This will adjust bearings to whole
    /// pixels and offset the resulting bitmap.
    pub align_whole_pixels: bool,
    /// This option will be ignored and set by _cpu or _gpu constructors
    pub cpu_rasterization: bool,
    /// Whether or to output a image instead of raw data. Only effects gpu rasterization
    pub raster_to_image: bool,
    /// Format used for the bitmap image.
    pub raster_image_format: Format,
}

impl ImtRasterOpts {
    pub fn sample_count(&self) -> usize {
        self.sample_quality.sample_count()
    }

    pub fn ray_count(&self) -> usize {
        self.fill_quality.ray_count()
    }
}

impl Default for ImtRasterOpts {
    fn default() -> Self {
        ImtRasterOpts {
            fill_quality: ImtFillQuality::Normal,
            sample_quality: ImtSampleQuality::Normal,
            align_whole_pixels: true,
            cpu_rasterization: false,
            raster_to_image: true,
            raster_image_format: Format::R8G8B8A8_UNORM,
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
    cache: Mutex<BTreeMap<(OrderedFloat<f32>, u16), RasterCacheState>>,
    gpu_raster_context: Option<GpuRasterContext>,
    cpu_raster_context: Option<CpuRasterContext>,
}

#[allow(dead_code)]
pub(crate) struct GpuRasterContext {
    pub device: Arc<Device>,
    pub queue: Arc<Queue>,
    pub glyph_cs: Arc<ShaderModule>,
    pub common_buf: Arc<DeviceLocalBuffer<glyph_cs::ty::Common>>,
    pub pipeline: Arc<ComputePipeline>,
    pub set_pool: Mutex<SingleLayoutDescSetPool>,
    pub raster_to_image: bool,
    pub raster_image_format: Format,
}

pub(crate) struct CpuRasterContext {
    pub samples: Vec<[f32; 2]>,
    pub rays: Vec<[f32; 2]>,
}

impl ImtRaster {
    pub fn new_gpu(
        device: Arc<Device>,
        queue: Arc<Queue>,
        mut opts: ImtRasterOpts,
    ) -> Result<Self, ImtError> {
        opts.cpu_rasterization = false;
        let glyph_cs = glyph_cs::load(device.clone()).unwrap();
        let mut samples_and_rays = [[0.0; 4]; 25];
        let sample_count = opts.sample_count();

        let w = (sample_count as f32).sqrt() as usize;
        let mut sar_i = 0;

        for x in 1..=w {
            for y in 1..=w {
                samples_and_rays[sar_i][0] = ((x as f32 / (w as f32 + 1.0)) * 2.0) - 1.0;
                samples_and_rays[sar_i][1] = ((y as f32 / (w as f32 + 1.0)) * 2.0) - 1.0;
                sar_i += 1;
            }
        }

        let ray_count = opts.ray_count();

        for i in 0..ray_count {
            let rad = (i as f32 * (360.0 / ray_count as f32)).to_radians();
            samples_and_rays[i][2] = rad.cos();
            samples_and_rays[i][3] = rad.sin();
        }

        let common_cpu_buf = CpuAccessibleBuffer::from_data(
            device.clone(),
            BufferUsage {
                transfer_src: true,
                ..BufferUsage::none()
            },
            false,
            glyph_cs::ty::Common {
                samples_and_rays,
                sample_count: sample_count as u32,
                ray_count: ray_count as u32,
            },
        )
        .unwrap();

        let common_dev_buf = DeviceLocalBuffer::new(
            device.clone(),
            BufferUsage {
                transfer_dst: true,
                uniform_buffer: true,
                ..BufferUsage::none()
            },
            iter::once(queue.family()),
        )
        .unwrap();

        let mut cmd_buf = AutoCommandBufferBuilder::primary(
            device.clone(),
            queue.family(),
            CommandBufferUsage::OneTimeSubmit,
        )
        .unwrap();

        cmd_buf
            .copy_buffer(CopyBufferInfo::buffers(
                common_cpu_buf,
                common_dev_buf.clone(),
            ))
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

        let pipeline = ComputePipeline::new(
            device.clone(),
            glyph_cs.entry_point("main").unwrap(),
            &(),
            None,
            |_| {},
        )
        .unwrap();

        let set_pool = SingleLayoutDescSetPool::new(pipeline.layout().set_layouts()[0].clone());
        let raster_to_image = opts.raster_to_image;
        let raster_image_format = opts.raster_image_format;

        Ok(ImtRaster {
            opts,
            cache: Mutex::new(BTreeMap::new()),
            gpu_raster_context: Some(GpuRasterContext {
                device,
                queue,
                glyph_cs,
                common_buf: common_dev_buf,
                pipeline,
                set_pool: Mutex::new(set_pool),
                raster_to_image,
                raster_image_format,
            }),
            cpu_raster_context: None,
        })
    }

    pub fn new_cpu(mut opts: ImtRasterOpts) -> Result<Self, ImtError> {
        opts.cpu_rasterization = true;
        let sample_count = opts.sample_count();
        let ray_count = opts.ray_count();
        let mut samples = Vec::with_capacity(sample_count);
        let mut rays = Vec::with_capacity(ray_count);
        let w = (sample_count as f32).sqrt() as usize;

        for x in 1..=w {
            for y in 1..=w {
                samples.push([
                    ((x as f32 / (w as f32 + 1.0)) * 2.0) - 1.0,
                    ((y as f32 / (w as f32 + 1.0)) * 2.0) - 1.0,
                ]);
            }
        }

        for i in 0..ray_count {
            let rad = (i as f32 * (360.0 / ray_count as f32)).to_radians();
            rays.push([rad.cos(), rad.sin()]);
        }

        Ok(ImtRaster {
            opts,
            cache: Mutex::new(BTreeMap::new()),
            gpu_raster_context: None,
            cpu_raster_context: Some(CpuRasterContext {
                samples,
                rays,
            }),
        })
    }

    pub fn sample_count(&self) -> usize {
        self.opts.sample_count()
    }

    pub fn ray_count(&self) -> usize {
        self.opts.ray_count()
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
            if let Some(cache_state) = cache_lk_op.as_mut().unwrap().get_mut(&(height_key, index)) {
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
                    let cache_state = cache_lk_op
                        .as_ref()
                        .unwrap()
                        .get(&(height_key, index))
                        .unwrap();

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
            cache_lk_op.as_mut().unwrap().insert(
                (height_key, index),
                RasterCacheState::Incomplete(Vec::new()),
            );

            // Drop the lock so other threads can keep doing things.
            cache_lk_op = None;

            let mut bitmap =
                ImtGlyphBitmap::new(parser, shaped.parsed.clone(), text_height, &self.opts);
            bitmap.create_outline();

            let raster_result = if self.opts.cpu_rasterization {
                bitmap.raster_cpu(self.cpu_raster_context.as_ref().unwrap())
            } else {
                bitmap.raster_gpu(self.gpu_raster_context.as_ref().unwrap())
            };

            if let Err(e) = raster_result {
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
            let old_state = cache_lk_op.as_mut().unwrap().insert(
                (height_key, index),
                RasterCacheState::Completed(bitmap.clone()),
            );

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
