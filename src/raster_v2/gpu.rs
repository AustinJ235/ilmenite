// TODO: Remove This
// #![allow(warnings)]

use std::collections::HashMap;
use std::iter;
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use ordered_float::OrderedFloat;
use parking_lot::Mutex;
use vulkano::buffer::{BufferUsage, CpuAccessibleBuffer, DeviceLocalBuffer, TypedBufferAccess};
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, BufferCopy, CommandBufferUsage, CopyBufferInfo, CopyBufferInfoTyped,
    PrimaryCommandBuffer, RenderPassBeginInfo, SubpassContents,
};
use vulkano::descriptor_set::{SingleLayoutDescSetPool, WriteDescriptorSet};
use vulkano::device::Queue;
use vulkano::format::{ClearValue, Format};
use vulkano::image::{AttachmentImage, ImageUsage};
use vulkano::pipeline::graphics::depth_stencil::{
    CompareOp, DepthStencilState, StencilOp, StencilOpState, StencilOps, StencilState,
};
use vulkano::pipeline::graphics::input_assembly::{InputAssemblyState, PrimitiveTopology};
use vulkano::pipeline::graphics::vertex_input::BuffersDefinition;
use vulkano::pipeline::graphics::viewport::{Viewport, ViewportState};
use vulkano::pipeline::{GraphicsPipeline, Pipeline, PipelineBindPoint, StateMode};
use vulkano::render_pass::{Framebuffer, FramebufferCreateInfo, RenderPass, Subpass};
use vulkano::sampler::{self, Sampler, SamplerCreateInfo};
use vulkano::shader::ShaderModule;
use vulkano::sync::GpuFuture;
use vulkano::{impl_vertex, single_pass_renderpass};

use super::{
    ImtBitmapData, ImtGlyphBitmap, ImtRaster, ImtRasterOps, ImtRasteredGlyph, ImtSubPixel,
};
use crate::image_view::ImtImageView;
use crate::{ImtError, ImtGeometry, ImtParsedGlyph, ImtParser, ImtShapedGlyph};

#[allow(dead_code)]
pub struct ImtRasterGpu {
    ops: ImtRasterOps,
    queue: Arc<Queue>,
    stencil_vs: Arc<ShaderModule>,
    stencil_fs: Arc<ShaderModule>,
    square_vs: Arc<ShaderModule>,
    sample_fs: Arc<ShaderModule>,
    blur_fs: Arc<ShaderModule>,
    stencil_renderpass: Arc<RenderPass>,
    sample_renderpass: Arc<RenderPass>,
    blur_renderpass: Arc<RenderPass>,
    stencil_pipeline: Arc<GraphicsPipeline>,
    sample_pipeline: Arc<GraphicsPipeline>,
    blur_pipeline: Arc<GraphicsPipeline>,
    square_vert_buf: Arc<DeviceLocalBuffer<[SquareVertex]>>,
    desc_set_pools: Mutex<DescSetPools>,
    glyph_cache: Mutex<GlyphCache>,
}

#[derive(Default)]
struct GlyphCache {
    vert_bufs: HashMap<u16, Option<Arc<DeviceLocalBuffer<[GlyphVertex]>>>>,
    bitmaps: HashMap<(u16, OrderedFloat<f32>), Arc<ImtGlyphBitmap>>,
}

struct DescSetPools {
    sample: SingleLayoutDescSetPool,
    blur: SingleLayoutDescSetPool,
}

impl ImtRasterGpu {
    pub fn new(queue: Arc<Queue>, ops: ImtRasterOps) -> Result<Self, ImtError> {
        // TODO: Handle Errors
        // TODO: Verify Format Compatibility

        let stencil_vs = stencil_vs::load(queue.device().clone()).unwrap();
        let stencil_fs = stencil_fs::load(queue.device().clone()).unwrap();
        let square_vs = square_vs::load(queue.device().clone()).unwrap();
        let sample_fs = sample_fs::load(queue.device().clone()).unwrap();
        let blur_fs = blur_fs::load(queue.device().clone()).unwrap();

        let stencil_renderpass = single_pass_renderpass!(
            queue.device().clone(),
            attachments: {
                stencil: {
                    load: Clear,
                    store: Store,
                    format: Format::S8_UINT,
                    samples: 1,
                }
            },
            pass: {
                color: [],
                depth_stencil: { stencil }
            }
        )
        .unwrap();

        let sample_renderpass = single_pass_renderpass!(
            queue.device().clone(),
            attachments: {
                color: {
                    load: Clear,
                    store: Store,
                    format: ops.bitmap_format,
                    samples: 1,
                }
            },
            pass: {
                color: [color],
                depth_stencil: {}
            }
        )
        .unwrap();

        let blur_renderpass = single_pass_renderpass!(
            queue.device().clone(),
            attachments: {
                color: {
                    load: Clear,
                    store: Store,
                    format: ops.bitmap_format,
                    samples: 1,
                }
            },
            pass: {
                color: [color],
                depth_stencil: {}
            }
        )
        .unwrap();

        let stencil_pipeline = GraphicsPipeline::start()
            .vertex_input_state(BuffersDefinition::new().vertex::<GlyphVertex>())
            .vertex_shader(stencil_vs.entry_point("main").unwrap(), ())
            .input_assembly_state(
                InputAssemblyState::new().topology(PrimitiveTopology::TriangleList),
            )
            .viewport_state(ViewportState::viewport_dynamic_scissor_irrelevant())
            .fragment_shader(stencil_fs.entry_point("main").unwrap(), ())
            .render_pass(Subpass::from(stencil_renderpass.clone(), 0).unwrap())
            .depth_stencil_state(DepthStencilState {
                depth: None,
                depth_bounds: None,
                stencil: Some(StencilState {
                    enable_dynamic: false,
                    front: StencilOpState {
                        ops: StateMode::Fixed(StencilOps {
                            fail_op: StencilOp::Invert,
                            pass_op: StencilOp::Invert,
                            depth_fail_op: StencilOp::Keep,
                            compare_op: CompareOp::Always,
                        }),
                        ..Default::default()
                    },
                    back: StencilOpState {
                        ops: StateMode::Fixed(StencilOps {
                            fail_op: StencilOp::Invert,
                            pass_op: StencilOp::Invert,
                            depth_fail_op: StencilOp::Keep,
                            compare_op: CompareOp::Always,
                        }),
                        ..Default::default()
                    },
                }),
            })
            .build(queue.device().clone())
            .unwrap();

        let sampler = Sampler::new(
            queue.device().clone(),
            SamplerCreateInfo {
                mag_filter: sampler::Filter::Nearest,
                ..Default::default()
            },
        )
        .unwrap();

        let sample_pipeline = GraphicsPipeline::start()
            .vertex_input_state(BuffersDefinition::new().vertex::<SquareVertex>())
            .vertex_shader(square_vs.entry_point("main").unwrap(), ())
            .input_assembly_state(
                InputAssemblyState::new().topology(PrimitiveTopology::TriangleList),
            )
            .viewport_state(ViewportState::viewport_dynamic_scissor_irrelevant())
            .fragment_shader(
                sample_fs.entry_point("main").unwrap(),
                sample_fs::SpecializationConstants {
                    ssaa: ops.ssaa.as_uint(),
                    subpixel: ops.subpixel.as_uint(),
                },
            )
            .render_pass(Subpass::from(sample_renderpass.clone(), 0).unwrap())
            .with_auto_layout(queue.device().clone(), |layout_create_infos| {
                let binding = layout_create_infos[0].bindings.get_mut(&0).unwrap();
                binding.immutable_samplers = vec![sampler.clone()];
            })
            .unwrap();

        let blur_pipeline = GraphicsPipeline::start()
            .vertex_input_state(BuffersDefinition::new().vertex::<SquareVertex>())
            .vertex_shader(square_vs.entry_point("main").unwrap(), ())
            .input_assembly_state(
                InputAssemblyState::new().topology(PrimitiveTopology::TriangleList),
            )
            .viewport_state(ViewportState::viewport_dynamic_scissor_irrelevant())
            .fragment_shader(blur_fs.entry_point("main").unwrap(), ())
            .render_pass(Subpass::from(blur_renderpass.clone(), 0).unwrap())
            .with_auto_layout(queue.device().clone(), |layout_create_infos| {
                let binding = layout_create_infos[0].bindings.get_mut(&0).unwrap();
                binding.immutable_samplers = vec![sampler];
            })
            .unwrap();

        let square_vert_buf = {
            let src = CpuAccessibleBuffer::from_iter(
                queue.device().clone(),
                BufferUsage::transfer_src(),
                false,
                [
                    SquareVertex {
                        position: [-1.0, -1.0],
                    },
                    SquareVertex {
                        position: [1.0, -1.0],
                    },
                    SquareVertex {
                        position: [1.0, 1.0],
                    },
                    SquareVertex {
                        position: [1.0, 1.0],
                    },
                    SquareVertex {
                        position: [-1.0, 1.0],
                    },
                    SquareVertex {
                        position: [-1.0, -1.0],
                    },
                ],
            )
            .unwrap();

            let dst: Arc<DeviceLocalBuffer<[SquareVertex]>> = DeviceLocalBuffer::array(
                queue.device().clone(),
                6,
                BufferUsage {
                    transfer_dst: true,
                    vertex_buffer: true,
                    ..BufferUsage::default()
                },
                iter::once(queue.family()),
            )
            .unwrap();

            let mut cmd_buf = AutoCommandBufferBuilder::primary(
                queue.device().clone(),
                queue.family(),
                CommandBufferUsage::OneTimeSubmit,
            )
            .unwrap();

            cmd_buf
                .copy_buffer(CopyBufferInfo::buffers(src, dst.clone()))
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

            dst
        };

        let sample_set_pool =
            SingleLayoutDescSetPool::new(sample_pipeline.layout().set_layouts()[0].clone());
        let blur_set_pool =
            SingleLayoutDescSetPool::new(blur_pipeline.layout().set_layouts()[0].clone());

        Ok(Self {
            ops,
            queue,
            stencil_vs,
            stencil_fs,
            square_vs,
            sample_fs,
            blur_fs,
            stencil_renderpass,
            sample_renderpass,
            blur_renderpass,
            stencil_pipeline,
            sample_pipeline,
            blur_pipeline,
            square_vert_buf,
            desc_set_pools: Mutex::new(DescSetPools {
                sample: sample_set_pool,
                blur: blur_set_pool,
            }),
            glyph_cache: Mutex::new(GlyphCache::default()),
        })
    }
}

impl ImtRaster for ImtRasterGpu {
    fn raster_shaped_glyphs(
        &self,
        parser: &ImtParser,
        text_height: f32,
        shaped_glyphs: Vec<ImtShapedGlyph>,
    ) -> Result<Vec<ImtRasteredGlyph>, ImtError> {
        let ord_text_height = OrderedFloat::from(text_height);
        let mut cache = self.glyph_cache.lock();

        let mut raster: Vec<(u16, usize)> = Vec::new();
        let mut upload: Vec<(u16, usize)> = Vec::new();

        for (shaped_i, glyph) in shaped_glyphs.iter().enumerate() {
            let glyph_i = glyph.parsed.inner.glyph_index;

            if !cache.bitmaps.contains_key(&(glyph_i, ord_text_height)) {
                raster.push((glyph_i, shaped_i));

                if !cache.vert_bufs.contains_key(&glyph_i) {
                    upload.push((glyph_i, shaped_i));
                }
            }
        }

        raster.sort_by_key(|(glyph_i, _)| *glyph_i);
        raster.dedup_by_key(|(glyph_i, _)| *glyph_i);
        upload.sort_by_key(|(glyph_i, _)| *glyph_i);
        upload.dedup_by_key(|(glyph_i, _)| *glyph_i);

        if !raster.is_empty() {
            if !upload.is_empty() {
                let mut src_vertexes: Vec<GlyphVertex> = Vec::new();
                let mut src_location: Vec<(u16, usize, usize)> = Vec::with_capacity(upload.len());

                for (glyph_i, shaped_i) in upload {
                    let start_i = src_vertexes.len();
                    let parsed = &shaped_glyphs[shaped_i].parsed;

                    for geo in parsed.geometry.iter() {
                        match geo {
                            ImtGeometry::Line(points) => {
                                src_vertexes.push(GlyphVertex {
                                    position: [-1.5; 2],
                                    coords: [0.0, 0.5],
                                });

                                src_vertexes.push(
                                    GlyphVertex {
                                        position: [points[0].x, points[0].y],
                                        coords: [0.0, 0.5],
                                    }
                                    .transform(parsed),
                                );

                                src_vertexes.push(
                                    GlyphVertex {
                                        position: [points[1].x, points[1].y],
                                        coords: [0.0, 0.5],
                                    }
                                    .transform(parsed),
                                );
                            },
                            ImtGeometry::Curve(points) => {
                                src_vertexes.push(GlyphVertex {
                                    position: [-1.5; 2],
                                    coords: [0.0, 0.5],
                                });

                                src_vertexes.push(
                                    GlyphVertex {
                                        position: [points[0].x, points[0].y],
                                        coords: [0.0, 0.5],
                                    }
                                    .transform(parsed),
                                );

                                src_vertexes.push(
                                    GlyphVertex {
                                        position: [points[2].x, points[2].y],
                                        coords: [0.0, 0.5],
                                    }
                                    .transform(parsed),
                                );

                                src_vertexes.push(
                                    GlyphVertex {
                                        position: [points[0].x, points[0].y],
                                        coords: [0.0, 0.0],
                                    }
                                    .transform(parsed),
                                );

                                src_vertexes.push(
                                    GlyphVertex {
                                        position: [points[1].x, points[1].y],
                                        coords: [1.0, 1.0],
                                    }
                                    .transform(parsed),
                                );

                                src_vertexes.push(
                                    GlyphVertex {
                                        position: [points[2].x, points[2].y],
                                        coords: [0.0, 0.0],
                                    }
                                    .transform(parsed),
                                );
                            },
                        }
                    }

                    let len = src_vertexes.len() - start_i;
                    src_location.push((glyph_i, start_i, len));
                }

                if src_vertexes.is_empty() {
                    for (glyph_i, ..) in src_location {
                        cache.vert_bufs.insert(glyph_i, None);
                    }
                } else {
                    let src = CpuAccessibleBuffer::from_iter(
                        self.queue.device().clone(),
                        BufferUsage::transfer_src(),
                        false,
                        src_vertexes,
                    )
                    .unwrap();

                    let mut cmd_buf = AutoCommandBufferBuilder::primary(
                        self.queue.device().clone(),
                        self.queue.family(),
                        CommandBufferUsage::OneTimeSubmit,
                    )
                    .unwrap();

                    for (glyph_i, start, len) in src_location {
                        if start == len {
                            cache.vert_bufs.insert(glyph_i, None);
                        } else {
                            let dst = DeviceLocalBuffer::array(
                                self.queue.device().clone(),
                                len as _,
                                BufferUsage {
                                    transfer_dst: true,
                                    vertex_buffer: true,
                                    ..BufferUsage::default()
                                },
                                iter::once(self.queue.family()),
                            )
                            .unwrap();

                            cmd_buf
                                .copy_buffer(CopyBufferInfoTyped {
                                    regions: [BufferCopy {
                                        src_offset: start as _,
                                        dst_offset: 0,
                                        size: len as _,
                                        ..BufferCopy::default()
                                    }]
                                    .into(),
                                    ..CopyBufferInfoTyped::buffers(src.clone(), dst.clone())
                                })
                                .unwrap();

                            cache.vert_bufs.insert(glyph_i, Some(dst));
                        }
                    }

                    // TODO: chain with raster future?
                    cmd_buf
                        .build()
                        .unwrap()
                        .execute(self.queue.clone())
                        .unwrap()
                        .then_signal_fence_and_flush()
                        .unwrap()
                        .wait(None)
                        .unwrap();
                }
            }

            let font_props = parser.font_props();
            let scaler = font_props.scaler * text_height;

            let mut cmd_buf = AutoCommandBufferBuilder::primary(
                self.queue.device().clone(),
                self.queue.family(),
                CommandBufferUsage::OneTimeSubmit,
            )
            .unwrap();

            let mut execute = false;

            for (glyph_i, shaped_i) in raster {
                match cache.vert_bufs.get(&glyph_i).unwrap() {
                    None => {
                        cache.bitmaps.insert(
                            (glyph_i, ord_text_height),
                            Arc::new(ImtGlyphBitmap {
                                width: 0,
                                height: 0,
                                bearing_x: 0.0,
                                bearing_y: 0.0,
                                text_height,
                                glyph_index: glyph_i,
                                data: ImtBitmapData::Empty,
                            }),
                        );
                    },
                    Some(vert_buf) => {
                        let parsed = &shaped_glyphs[shaped_i].parsed;
                        let width = ((parsed.max_x - parsed.min_x) * scaler).ceil() as u32;
                        let height = ((parsed.max_y - parsed.min_y) * scaler).ceil() as u32;

                        if width == 0 || height == 0 {
                            cache.bitmaps.insert(
                                (glyph_i, ord_text_height),
                                Arc::new(ImtGlyphBitmap {
                                    width: 0,
                                    height: 0,
                                    bearing_x: 0.0,
                                    bearing_y: 0.0,
                                    text_height,
                                    glyph_index: glyph_i,
                                    data: ImtBitmapData::Empty,
                                }),
                            );

                            continue;
                        }

                        let extent = [width, height];
                        let ssaa = self.ops.ssaa.as_uint();

                        let stencil_extent = match self.ops.subpixel {
                            ImtSubPixel::None => [width * ssaa, height * ssaa],
                            ImtSubPixel::RGB | ImtSubPixel::BGR => {
                                [width * ssaa * 3, height * ssaa]
                            },
                            ImtSubPixel::VRGB | ImtSubPixel::VBGR => {
                                [width * ssaa, height * ssaa * 3]
                            },
                        };

                        let stencil_buffer = ImtImageView::from_attachment(
                            AttachmentImage::with_usage(
                                self.queue.device().clone(),
                                stencil_extent,
                                Format::S8_UINT,
                                ImageUsage {
                                    depth_stencil_attachment: true,
                                    sampled: true,
                                    ..ImageUsage::none()
                                },
                            )
                            .unwrap(),
                        )
                        .unwrap();

                        let stencil_framebuffer = Framebuffer::new(
                            self.stencil_renderpass.clone(),
                            FramebufferCreateInfo {
                                attachments: vec![stencil_buffer.clone()],
                                ..Default::default()
                            },
                        )
                        .unwrap();

                        let sample_image = ImtImageView::from_attachment(
                            AttachmentImage::with_usage(
                                self.queue.device().clone(),
                                extent,
                                self.ops.bitmap_format,
                                ImageUsage {
                                    color_attachment: true,
                                    sampled: true,
                                    ..ImageUsage::none()
                                },
                            )
                            .unwrap(),
                        )
                        .unwrap();

                        let sample_framebuffer = Framebuffer::new(
                            self.sample_renderpass.clone(),
                            FramebufferCreateInfo {
                                attachments: vec![sample_image.clone()],
                                ..Default::default()
                            },
                        )
                        .unwrap();

                        let (blur_image_op, blur_framebuffer_op) =
                            if self.ops.subpixel == ImtSubPixel::None {
                                (None, None)
                            } else {
                                let blur_image = ImtImageView::from_attachment(
                                    AttachmentImage::with_usage(
                                        self.queue.device().clone(),
                                        extent,
                                        self.ops.bitmap_format,
                                        ImageUsage {
                                            color_attachment: true,
                                            transfer_src: true,
                                            sampled: true,
                                            ..ImageUsage::none()
                                        },
                                    )
                                    .unwrap(),
                                )
                                .unwrap();

                                let blur_framebuffer = Framebuffer::new(
                                    self.blur_renderpass.clone(),
                                    FramebufferCreateInfo {
                                        attachments: vec![blur_image.clone()],
                                        ..Default::default()
                                    },
                                )
                                .unwrap();

                                (Some(blur_image), Some(blur_framebuffer))
                            };

                        let (sample_set, blur_set_op) = {
                            let mut desc_set_pools = self.desc_set_pools.lock();

                            let sample_set = desc_set_pools
                                .sample
                                .next([WriteDescriptorSet::image_view(0, stencil_buffer.clone())])
                                .unwrap();

                            let blur_set_op = if self.ops.subpixel == ImtSubPixel::None {
                                None
                            } else {
                                Some(
                                    desc_set_pools
                                        .blur
                                        .next([WriteDescriptorSet::image_view(
                                            0,
                                            sample_image.clone(),
                                        )])
                                        .unwrap(),
                                )
                            };

                            (sample_set, blur_set_op)
                        };

                        cmd_buf
                            // Begin Stencil
                            .begin_render_pass(
                                RenderPassBeginInfo {
                                    clear_values: vec![Some(ClearValue::Stencil(0))],
                                    ..RenderPassBeginInfo::framebuffer(stencil_framebuffer.clone())
                                },
                                SubpassContents::Inline,
                            )
                            .unwrap()
                            .set_viewport(0, iter::once(Viewport {
                                origin: [0.0; 2],
                                dimensions: [stencil_extent[0] as f32, stencil_extent[1] as f32],
                                depth_range: 0.0..1.0,
                            }))
                            .bind_pipeline_graphics(self.stencil_pipeline.clone())
                            .bind_vertex_buffers(0, vert_buf.clone())
                            .draw(vert_buf.len() as u32, 1, 0, 0)
                            .unwrap()
                            .end_render_pass()
                            .unwrap()
                            // Begin Sample
                            .begin_render_pass(
                                RenderPassBeginInfo {
                                    clear_values: vec![Some(ClearValue::Float([0.0; 4]))],
                                    ..RenderPassBeginInfo::framebuffer(sample_framebuffer.clone())
                                },
                                SubpassContents::Inline,
                            )
                            .unwrap()
                            .set_viewport(0, iter::once(Viewport {
                                origin: [0.0; 2],
                                dimensions: [extent[0] as f32, extent[1] as f32],
                                depth_range: 0.0..1.0,
                            }))
                            .bind_pipeline_graphics(self.sample_pipeline.clone())
                            .push_constants(self.sample_pipeline.layout().clone(), 0, sample_fs::ty::GlyphInfo {
                                width: extent[0],
                                height: extent[1],
                            })
                            .bind_descriptor_sets(
                                PipelineBindPoint::Graphics,
                                self.sample_pipeline.layout().clone(),
                                0,
                                sample_set,
                            )
                            .bind_vertex_buffers(0, self.square_vert_buf.clone())
                            .draw(self.square_vert_buf.len() as u32, 1, 0, 0)
                            .unwrap()
                            .end_render_pass()
                            .unwrap();

                        if self.ops.subpixel != ImtSubPixel::None {
                            cmd_buf
                                // Begin Blur
                                .begin_render_pass(
                                    RenderPassBeginInfo {
                                        clear_values: vec![Some(ClearValue::Float([0.0; 4]))],
                                        ..RenderPassBeginInfo::framebuffer(blur_framebuffer_op.unwrap())
                                    },
                                    SubpassContents::Inline,
                                )
                                .unwrap()
                                .set_viewport(0, iter::once(Viewport {
                                    origin: [0.0; 2],
                                    dimensions: [extent[0] as f32, extent[1] as f32],
                                    depth_range: 0.0..1.0,
                                }))
                                .bind_pipeline_graphics(self.blur_pipeline.clone())
                                .push_constants(self.blur_pipeline.layout().clone(), 0, blur_fs::ty::GlyphInfo {
                                    width: extent[0],
                                })
                                .bind_descriptor_sets(
                                    PipelineBindPoint::Graphics,
                                    self.blur_pipeline.layout().clone(),
                                    0,
                                    blur_set_op.unwrap(),
                                )
                                .bind_vertex_buffers(0, self.square_vert_buf.clone())
                                .draw(self.square_vert_buf.len() as u32, 1, 0, 0)
                                .unwrap()
                                .end_render_pass()
                                .unwrap();
                        }

                        execute = true;

                        let data = if self.ops.subpixel == ImtSubPixel::None {
                            ImtBitmapData::Image(sample_image)
                        } else {
                            ImtBitmapData::Image(blur_image_op.unwrap())
                        };

                        cache.bitmaps.insert(
                            (glyph_i, ord_text_height),
                            Arc::new(ImtGlyphBitmap {
                                width: extent[0],
                                height: extent[1],
                                bearing_x: (parsed.min_x * scaler).floor(),
                                bearing_y: ((font_props.ascender - parsed.max_y) * scaler).floor(),
                                text_height,
                                glyph_index: glyph_i,
                                data,
                            }),
                        );
                    },
                }
            }

            if execute {
                cmd_buf
                    .build()
                    .unwrap()
                    .execute(self.queue.clone())
                    .unwrap()
                    .then_signal_fence_and_flush()
                    .unwrap()
                    .wait(None)
                    .unwrap();
            }
        }

        Ok(shaped_glyphs
            .into_iter()
            .map(|shaped| {
                let glyph_i = shaped.parsed.inner.glyph_index;

                ImtRasteredGlyph {
                    shaped,
                    bitmap: cache
                        .bitmaps
                        .get(&(glyph_i, ord_text_height))
                        .unwrap()
                        .clone(),
                }
            })
            .collect())
    }
}

mod square_vs {
    vulkano_shaders::shader! {
        ty: "vertex",
        src: "
            #version 450

            layout(location = 0) in vec2 position;
            layout(location = 0) out vec2 out_coords;

            void main() {
                out_coords = vec2(
                    (position.x + 1.0) / 2.0,
                    (position.y + 1.0) / 2.0
                );

                gl_Position = vec4(position, 0.0, 1.0);
            }
        "
    }
}

#[derive(Pod, Zeroable, Clone, Copy, Debug, Default)]
#[repr(C)]
struct GlyphVertex {
    position: [f32; 2],
    coords: [f32; 2],
}

impl GlyphVertex {
    fn transform(mut self, glyph: &ImtParsedGlyph) -> Self {
        self.position[0] =
            (((self.position[0] - glyph.min_x) / (glyph.max_x - glyph.min_x)) * 2.0) - 1.0;
        self.position[1] =
            -((((self.position[1] - glyph.min_y) / (glyph.max_y - glyph.min_y)) * 2.0) - 1.0);
        self
    }
}

impl_vertex!(GlyphVertex, position, coords);

mod stencil_vs {
    vulkano_shaders::shader! {
        ty: "vertex",
        src: "
            #version 450

            layout(location = 0) in vec2 position;
            layout(location = 1) in vec2 coords;
            layout(location = 0) out vec2 out_coords;

            void main() {
                gl_Position = vec4(position, 0.0, 1.0);
                out_coords = coords;
            }
        "
    }
}

mod stencil_fs {
    vulkano_shaders::shader! {
        ty: "fragment",
        src: "
            #version 450

            layout(location = 0) in vec2 coords;

            void main() {
                if(pow((coords.x / 2.0) + coords.y, 2) > coords.y + 0.1) {
                    discard;
                }
            }
        "
    }
}

#[derive(Pod, Zeroable, Clone, Copy, Debug, Default)]
#[repr(C)]
struct SquareVertex {
    position: [f32; 2],
}

impl_vertex!(SquareVertex, position);

mod sample_fs {
    vulkano_shaders::shader! {
        ty: "fragment",
        src: "
            #version 450

            layout(constant_id = 0) const uint ssaa = 4;
            layout(constant_id = 1) const uint subpixel = 1;

            layout(push_constant) uniform GlyphInfo {
                uint width;
                uint height;
            } info;

            layout(set = 0, binding = 0) uniform usampler2D stencil;

            layout(location = 0) in vec2 coords;
            layout(location = 0) out vec4 color;

            void main() {
                if(subpixel == 1) {
                    float samples = float(pow(ssaa, 2));
                    float sampleStrideX = (1.0 / (float(info.width) * float(ssaa) * 3.0));
                    float sampleStrideY = (1.0 / (float(info.height) * float(ssaa)));
                    float subPixelStride = sampleStrideX * 3;
                    vec3 rgbColor = vec3(0.0);

                    for(uint x = 0; x < ssaa; x++) {
                        for(uint y = 0; y < ssaa; y++) {
                            vec2 rCoords = coords
                                + vec2(
                                    float(x) * sampleStrideX,
                                    float(y) * sampleStrideY
                                );
                            vec2 gCoords = coords
                                + vec2(
                                    subPixelStride + (float(x) * sampleStrideX),
                                    subPixelStride + (float(y) * sampleStrideY)
                                );
                            vec2 bCoords = coords
                                + vec2(
                                    (subPixelStride * 2.0) + (float(x) * sampleStrideX),
                                    (subPixelStride * 2.0)  + (float(y) * sampleStrideY)
                                );
                            
                            uint stencilR = texture(stencil, rCoords).r;
                            uint stencilG = texture(stencil, gCoords).r;
                            uint stencilB = texture(stencil, bCoords).r;

                            if(stencilR > 128) {
                                rgbColor.r += 1.0;
                            }
                            
                            if(stencilG > 128) {
                                rgbColor.g += 1.0;
                            }

                            if(stencilB > 128) {
                                rgbColor.b += 1.0;
                            }
                        }
                    }

                    rgbColor /= samples;
                    color = vec4(rgbColor, 1.0);
                }
            }
        "
    }
}

mod blur_fs {
    vulkano_shaders::shader! {
        ty: "fragment",
        src: "
            #version 450

            layout(push_constant) uniform GlyphInfo {
                uint width;
            } info;

            layout(set = 0, binding = 0) uniform sampler2D sampled;

            layout(location = 0) in vec2 coords;
            layout(location = 0) out vec4 color;

            void main() {
                float pixelStrideX = 1.0 / float(info.width);
                float leftSubG = texture(sampled, coords - vec2(pixelStrideX, 0.0)).g;
                float rightSubR = texture(sampled, coords + vec2(pixelStrideX, 0.0)).r;
                vec4 thisColor = texture(sampled, coords).rgba;

                color = vec4(
                    (leftSubG + thisColor.r + thisColor.g) / 3.0,
                    (thisColor.r + thisColor.g + thisColor.b) / 3.0,
                    (thisColor.g + thisColor.b + rightSubR) / 3.0,
                    thisColor.a
                );
            }
        "
    }
}
