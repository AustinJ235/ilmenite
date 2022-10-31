use std::iter;
use std::sync::Arc;

use vulkano::buffer::cpu_access::CpuAccessibleBuffer;
use vulkano::buffer::BufferUsage;
use vulkano::command_buffer::{
    AutoCommandBufferBuilder, CommandBufferUsage, CopyImageToBufferInfo,
    PrimaryCommandBufferAbstract,
};
use vulkano::descriptor_set::WriteDescriptorSet;
use vulkano::image::{ImageCreateFlags, ImageDimensions, ImageUsage, StorageImage};
use vulkano::pipeline::{Pipeline, PipelineBindPoint};
use vulkano::sync::GpuFuture;

use crate::raster::{CpuRasterContext, GpuRasterContext};
use crate::shaders::glyph_cs;
use crate::{
    ImtError, ImtGeometry, ImtImageView, ImtParsedGlyph, ImtParser, ImtPoint, ImtRasterOpts,
};

#[derive(Clone)]
pub enum ImtBitmapData {
    Empty,
    LRGBA(Arc<Vec<f32>>),
    Image(Arc<ImtImageView>),
}

#[derive(Debug, Clone)]
pub struct ImtBitmapMetrics {
    pub width: u32,
    pub height: u32,
    pub bearing_x: f32,
    pub bearing_y: f32,
}

/// Data is Linear RGBA
#[derive(Clone)]
pub struct ImtGlyphBitmap {
    parsed: Arc<ImtParsedGlyph>,
    metrics: ImtBitmapMetrics,
    lines: Vec<(ImtPoint, ImtPoint)>,
    scaler: f32,
    offset_x: f32,
    offset_y: f32,
    data: Option<ImtBitmapData>,
}

fn expand_round(val: f32, direction: bool) -> f32 {
    if direction {
        if val.is_sign_positive() {
            val.ceil() + 1.0
        } else {
            val.trunc() + 1.0
        }
    } else {
        if val.is_sign_positive() {
            val.trunc() - 1.0
        } else {
            val.ceil() - 1.0
        }
    }
}

impl ImtGlyphBitmap {
    pub fn new(
        parser: &ImtParser,
        parsed: Arc<ImtParsedGlyph>,
        text_height: f32,
        raster_opts: &ImtRasterOpts,
    ) -> ImtGlyphBitmap {
        let font_props = parser.font_props();
        let scaler = font_props.scaler * text_height;

        let mut bearing_x = parsed.min_x * scaler;
        let mut bearing_y = (font_props.ascender - parsed.max_y) * scaler;

        let (offset_x, offset_y) = if raster_opts.align_whole_pixels {
            let offset_x = (bearing_x - bearing_x.ceil()) + 1.0;
            bearing_x = bearing_x.ceil();
            let offset_y = -(bearing_y - bearing_y.ceil()) - 1.0;
            bearing_y = bearing_y.ceil();
            (offset_x, offset_y)
        } else {
            (0.0, 0.0)
        };

        let height = (expand_round(parsed.max_y * scaler, true)
            - expand_round(parsed.min_y * scaler, false)) as u32
            + 1;
        let width = (expand_round(parsed.max_x * scaler, true)
            - expand_round(parsed.min_x * scaler, false)) as u32
            + 1;

        ImtGlyphBitmap {
            parsed,
            metrics: ImtBitmapMetrics {
                width,
                height,
                bearing_x,
                bearing_y,
            },
            offset_x,
            offset_y,
            data: None,
            lines: Vec::new(),
            scaler,
        }
    }

    pub fn data(&self) -> Option<ImtBitmapData> {
        self.data.clone()
    }

    pub fn metrics(&self) -> ImtBitmapMetrics {
        self.metrics.clone()
    }

    pub(crate) fn raster_cpu(&mut self, context: &CpuRasterContext) -> Result<(), ImtError> {
        if self.metrics.width == 0 || self.metrics.height == 0 || self.lines.is_empty() {
            self.data = Some(ImtBitmapData::Empty);
            return Ok(());
        }

        let ray_count = context.rays.len();
        let sample_count = context.samples.len();

        let ray_intersects =
            |l1p1: [f32; 2], l1p2: [f32; 2], l2p1: [f32; 2], l2p2: [f32; 2]| -> Option<[f32; 2]> {
                let r = [l1p2[0] - l1p1[0], l1p2[1] - l1p1[1]];
                let s = [l2p2[0] - l2p1[0], l2p2[1] - l2p1[1]];
                let det = (r[0] * s[1]) - (r[1] * s[0]);
                let u = (((l2p1[0] - l1p1[0]) * r[1]) - ((l2p1[1] - l1p1[1]) * r[0])) / det;
                let t = (((l2p1[0] - l1p1[0]) * s[1]) - ((l2p1[1] - l1p1[1]) * s[0])) / det;

                if t >= 0.0 && t <= 1.0 && u >= 0.0 && u <= 1.0 {
                    Some([(l1p1[0] + r[0]) * t, (l1p1[1] + r[1]) * t])
                } else {
                    None
                }
            };

        let cell_height = self.scaler / (sample_count as f32).sqrt();
        let cell_width = cell_height / 3.0;

        let sample_filled = |ray_src: [f32; 2], ray_len: f32| -> Option<f32> {
            let mut rays_filled = 0;
            let mut ray_fill_amt = 0.0;

            for ray in context.rays.iter() {
                let mut hits = 0_isize;

                let ray_dest = [
                    ray_src[0] + (ray[0] * ray_len),
                    ray_src[1] + (ray[1] * ray_len),
                ];

                let ray_angle = (ray[1] / ray[0]).atan();
                let mut ray_max_dist = (cell_width / 2.0) / ray_angle.cos();

                if ray_max_dist > (cell_height / 2.0) {
                    ray_max_dist = (cell_height / 2.0) / (1.570796327 - ray_angle).cos();
                }

                let mut ray_min_dist = ray_max_dist;

                for line in self.lines.iter() {
                    match ray_intersects(
                        ray_src,
                        ray_dest,
                        [line.0.x, line.0.y],
                        [line.1.x, line.1.y],
                    ) {
                        Some(intersect_point) => {
                            let dist = ((ray_src[0] - intersect_point[0]).powi(2)
                                + (ray_src[1] - intersect_point[1]).powi(2))
                            .sqrt();

                            if dist < ray_min_dist {
                                ray_min_dist = dist;
                            }

                            hits += 1;
                        },
                        None => (),
                    }
                }

                if hits % 2 != 0 {
                    rays_filled += 1;
                    ray_fill_amt += ray_min_dist / ray_max_dist;
                }
            }

            if rays_filled >= ray_count / 2 {
                Some(ray_fill_amt / rays_filled as f32)
            } else {
                None
            }
        };

        let transform_coords =
            |coords: [usize; 2], offset_i: usize, offset: [f32; 2]| -> [f32; 2] {
                let mut coords = [coords[0] as f32, coords[1] as f32 * -1.0];
                coords[0] -= self.offset_x;
                coords[1] -= self.offset_y;
                coords[0] += context.samples[offset_i][0];
                coords[1] += context.samples[offset_i][1];
                coords[0] += offset[0];
                coords[1] += offset[1];
                coords[0] /= self.scaler;
                coords[1] /= self.scaler;
                coords[0] += self.parsed.min_x;
                coords[1] += self.parsed.max_y;
                coords
            };

        let get_value = |coords: [usize; 2], offset: [f32; 2], ray_len: f32| -> f32 {
            let mut fill_amt_sum = 0.0;

            for i in 0..sample_count {
                if let Some(fill_amt) = sample_filled(transform_coords(coords, i, offset), ray_len)
                {
                    fill_amt_sum += fill_amt;
                }
            }

            fill_amt_sum / sample_count as f32
        };

        let mut bitmap: Vec<f32> =
            Vec::with_capacity((self.metrics.width * self.metrics.height * 4) as usize);
        bitmap.resize((self.metrics.width * self.metrics.height * 4) as usize, 0.0);
        let ray_len = ((self.metrics.width as f32 / self.scaler).powi(2)
            + (self.metrics.height as f32 / self.scaler).powi(2))
        .sqrt();

        for x in 0..self.metrics.width {
            for y in 0..self.metrics.height {
                let rindex = (((y * self.metrics.width) + x) * 4) as usize;
                let r = get_value([x as usize, y as usize], [1.0 / 6.0, 0.0], ray_len);
                let g = get_value([x as usize, y as usize], [3.0 / 6.0, 0.0], ray_len);
                let b = get_value([x as usize, y as usize], [5.0 / 6.0, 0.0], ray_len);
                let a = (r + g + b) / 3.0;
                bitmap[rindex] = r / a;
                bitmap[rindex + 1] = g / a;
                bitmap[rindex + 2] = b / a;
                bitmap[rindex + 3] = a;
            }
        }

        self.data = Some(ImtBitmapData::LRGBA(Arc::new(bitmap)));
        Ok(())
    }

    pub(crate) fn raster_gpu(&mut self, context: &GpuRasterContext) -> Result<(), ImtError> {
        if self.metrics.width == 0 || self.metrics.height == 0 || self.lines.is_empty() {
            self.data = Some(ImtBitmapData::Empty);
            return Ok(());
        }

        let glyph_buf: Arc<CpuAccessibleBuffer<glyph_cs::ty::Glyph>> =
            CpuAccessibleBuffer::from_data(
                &context.mem_alloc,
                BufferUsage {
                    uniform_buffer: true,
                    ..BufferUsage::empty()
                },
                false,
                glyph_cs::ty::Glyph {
                    scaler: self.scaler,
                    width: self.metrics.width,
                    height: self.metrics.height,
                    line_count: self.lines.len() as u32,
                    bounds: [
                        self.parsed.min_x,
                        self.parsed.max_x,
                        self.parsed.min_y,
                        self.parsed.max_y,
                    ],
                    offset: [self.offset_x, self.offset_y],
                },
            )
            .unwrap();

        let bitmap_img = ImtImageView::from_storage(
            StorageImage::with_usage(
                &context.mem_alloc,
                ImageDimensions::Dim2d {
                    width: self.metrics.width,
                    height: self.metrics.height,
                    array_layers: 1,
                },
                context.raster_image_format,
                ImageUsage {
                    transfer_src: true,
                    storage: true,
                    ..ImageUsage::empty()
                },
                ImageCreateFlags::empty(),
                iter::once(context.queue.queue_family_index()),
            )
            .unwrap(),
        )
        .unwrap();

        let line_buf: Arc<CpuAccessibleBuffer<[[f32; 4]]>> = CpuAccessibleBuffer::from_iter(
            &context.mem_alloc,
            BufferUsage {
                storage_buffer: true,
                ..BufferUsage::empty()
            },
            false,
            self.lines
                .iter()
                .map(|line| [line.0.x, line.0.y, line.1.x, line.1.y]),
        )
        .unwrap();

        let descriptor_set = context
            .set_pool
            .lock()
            .next(
                vec![
                    WriteDescriptorSet::buffer(0, context.common_buf.clone()),
                    WriteDescriptorSet::buffer(1, glyph_buf),
                    WriteDescriptorSet::image_view(2, bitmap_img.clone()),
                    WriteDescriptorSet::buffer(3, line_buf),
                ]
                .into_iter(),
            )
            .unwrap();

        let mut cmd_buf = AutoCommandBufferBuilder::primary(
            &context.cmd_alloc,
            context.queue.queue_family_index(),
            CommandBufferUsage::OneTimeSubmit,
        )
        .unwrap();

        cmd_buf
            .bind_pipeline_compute(context.pipeline.clone())
            .bind_descriptor_sets(
                PipelineBindPoint::Compute,
                context.pipeline.layout().clone(),
                0,
                descriptor_set,
            )
            .dispatch([self.metrics.width, self.metrics.height, 1])
            .unwrap();

        cmd_buf
            .build()
            .unwrap()
            .execute(context.queue.clone())
            .unwrap()
            .then_signal_fence_and_flush()
            .unwrap()
            .wait(None)
            .unwrap();

        if !context.raster_to_image {
            let len = (self.metrics.width * self.metrics.height * 4) as u64;
            let bitmap_buf: Arc<CpuAccessibleBuffer<[u8]>> = unsafe {
                CpuAccessibleBuffer::uninitialized_array(
                    &context.mem_alloc,
                    len,
                    BufferUsage {
                        transfer_dst: true,
                        ..BufferUsage::empty()
                    },
                    true,
                )
                .unwrap()
            };

            let mut cmd_buf = AutoCommandBufferBuilder::primary(
                &context.cmd_alloc,
                context.queue.queue_family_index(),
                CommandBufferUsage::OneTimeSubmit,
            )
            .unwrap();

            cmd_buf
                .copy_image_to_buffer(CopyImageToBufferInfo::image_buffer(
                    bitmap_img,
                    bitmap_buf.clone(),
                ))
                .unwrap();

            cmd_buf
                .build()
                .unwrap()
                .execute(context.queue.clone())
                .unwrap()
                .then_signal_fence_and_flush()
                .unwrap()
                .wait(None)
                .unwrap();

            self.data = Some(ImtBitmapData::LRGBA(Arc::new(
                bitmap_buf
                    .read()
                    .unwrap()
                    .iter()
                    .map(|v| *v as f32 / u8::max_value() as f32)
                    .collect(),
            )));
        } else {
            self.data = Some(ImtBitmapData::Image(bitmap_img));
        }

        Ok(())
    }

    pub(crate) fn create_outline(&mut self) {
        for geometry in self.parsed.geometry.clone() {
            self.draw_geometry(&geometry);
        }
    }

    fn draw_geometry(&mut self, geo: &ImtGeometry) {
        match geo {
            &ImtGeometry::Line(ref points) => self.draw_line(&points[0], &points[1]),
            &ImtGeometry::Curve(ref points) => self.draw_curve(&points[0], &points[1], &points[2]),
        }
    }

    fn draw_line(&mut self, point_a: &ImtPoint, point_b: &ImtPoint) {
        self.lines.push((
            ImtPoint {
                x: point_a.x,
                y: point_a.y,
            },
            ImtPoint {
                x: point_b.x,
                y: point_b.y,
            },
        ));
    }

    fn draw_curve(&mut self, point_a: &ImtPoint, point_b: &ImtPoint, point_c: &ImtPoint) {
        let mut length = 0.0;
        let mut last_point = point_a.clone();
        let mut steps = 10_usize;

        for s in 1..=steps {
            let t = s as f32 / steps as f32;
            let next_point = ImtPoint {
                x: ((1.0 - t).powi(2) * point_a.x)
                    + (2.0 * (1.0 - t) * t * point_b.x)
                    + (t.powi(2) * point_c.x),
                y: ((1.0 - t).powi(2) * point_a.y)
                    + (2.0 * (1.0 - t) * t * point_b.y)
                    + (t.powi(2) * point_c.y),
            };

            length += last_point.dist(&next_point);
            last_point = next_point;
        }

        steps = (length * self.scaler * 2.0).ceil() as usize;

        if steps < 3 {
            steps = 3;
        }

        last_point = point_a.clone();

        for s in 1..=steps {
            let t = s as f32 / steps as f32;
            let next_point = ImtPoint {
                x: ((1.0 - t).powi(2) * point_a.x)
                    + (2.0 * (1.0 - t) * t * point_b.x)
                    + (t.powi(2) * point_c.x),
                y: ((1.0 - t).powi(2) * point_a.y)
                    + (2.0 * (1.0 - t) * t * point_b.y)
                    + (t.powi(2) * point_c.y),
            };

            self.draw_line(&last_point, &next_point);
            last_point = next_point;
        }
    }
}
