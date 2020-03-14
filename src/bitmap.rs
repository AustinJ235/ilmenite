use crate::{
	shaders::glyph_base_fs, ImtError, ImtGeometry, ImtParsedGlyph, ImtParser, ImtPoint,
	ImtRaster, ImtShaderVert,
};

use std::sync::Arc;
use vulkano::{
	buffer::{cpu_access::CpuAccessibleBuffer, BufferUsage},
	command_buffer::{AutoCommandBufferBuilder, CommandBuffer},
	descriptor::descriptor_set::PersistentDescriptorSet,
	format::Format,
	framebuffer::{Framebuffer, Subpass},
	image::{attachment::AttachmentImage, ImageUsage},
	pipeline::{input_assembly::PrimitiveTopology, viewport::Viewport, GraphicsPipeline},
	sync::GpuFuture,
};

#[derive(Clone)]
pub struct ImtGlyphBitmap {
	parsed: Arc<ImtParsedGlyph>,
	pub width: u32,
	pub height: u32,
	pub bearing_x: f32,
	pub bearing_y: f32,
	pub data: Vec<Vec<f32>>,
	lines: Vec<(ImtPoint, ImtPoint)>,
	scaler: f32,
	pixel_align_offset_x: f32,
	pixel_align_offset_y: f32,
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
		parser: &mut ImtParser,
		parsed: Arc<ImtParsedGlyph>,
		text_height: f32,
	) -> ImtGlyphBitmap {
		let font_props = parser.font_props();
		let scaler = font_props.scaler * text_height;
		let mut bearing_x = parsed.min_x * scaler;
		let mut bearing_y = (font_props.ascender - parsed.max_y) * scaler;

		let pixel_align_offset_x = (bearing_x.round() - bearing_x)
			+ expand_round(parsed.min_x * scaler, false)
			- (parsed.min_x * scaler);
		let pixel_align_offset_y = (bearing_y.round() - bearing_y)
			- expand_round(parsed.min_y * scaler, true)
			+ (parsed.min_y * scaler);

		bearing_x = bearing_x.round();
		bearing_y = bearing_y.round();

		let height = (expand_round(parsed.max_y * scaler, true)
			- expand_round(parsed.min_y * scaler, false)) as u32;
		let width = (expand_round(parsed.max_x * scaler, true)
			- expand_round(parsed.min_x * scaler, false)) as u32;

		let mut data = Vec::with_capacity(width as usize);
		data.resize_with(width as usize, || {
			let mut col = Vec::with_capacity(height as usize);
			col.resize(height as usize, 0.0);
			col
		});

		ImtGlyphBitmap {
			parsed,
			width,
			height,
			bearing_x,
			bearing_y,
			data,
			lines: Vec::new(),
			pixel_align_offset_x,
			pixel_align_offset_y,
			scaler,
		}
	}

	pub(crate) fn raster(&mut self, raster: &ImtRaster) -> Result<(), ImtError> {
		if self.width == 0 || self.height == 0 {
			return Ok(());
		}

		let mut line_data = glyph_base_fs::ty::LineData {
			lines: [[0.0; 4]; 1024],
			count: 0,
			width: self.width,
			height: self.height,
			bounds: [
				self.parsed.min_x,
				self.parsed.max_x,
				self.parsed.min_y,
				self.parsed.max_y,
			],
			pixel_align_offset: [
				self.pixel_align_offset_x,
				self.pixel_align_offset_y,
				0.0,
				0.0,
			],
			scaler: self.scaler,
			_dummy0: [0; 4],
		};

		for (pt_a, pt_b) in &self.lines {
			let i = line_data.count;
			line_data.lines[i as usize] = [pt_a.x, pt_a.y, pt_b.x, pt_b.y];
			line_data.count += 1;
		}

		let line_data_buf = CpuAccessibleBuffer::from_data(
			raster.device.clone(),
			BufferUsage::all(), // TODO: Specific Usage
			false,
			line_data,
		)
		.unwrap();

		let p1_out_image = AttachmentImage::with_usage(
			raster.device.clone(),
			[self.width, self.height],
			Format::R8Unorm,
			ImageUsage {
				transfer_source: true,
				color_attachment: true,
				sampled: true,
				..vulkano::image::ImageUsage::none()
			},
		)
		.unwrap();

		let p1_render_pass = Arc::new(
			vulkano::single_pass_renderpass!(
				raster.device.clone(),
				attachments: {
					color: {
						load: Clear,
						store: Store,
						format: Format::R8Unorm,
						samples: 1,
					}
				},
				pass: {
					color: [color],
					depth_stencil: {}
				}
			)
			.unwrap(),
		);

		let p1_pipeline = Arc::new(
			GraphicsPipeline::start()
				.vertex_input_single_buffer::<ImtShaderVert>()
				.vertex_shader(raster.square_vs.main_entry_point(), ())
				.fragment_shader(raster.glyph_base_fs.main_entry_point(), ())
				.primitive_topology(PrimitiveTopology::TriangleList)
				.render_pass(Subpass::from(p1_render_pass.clone(), 0).unwrap())
				.viewports(::std::iter::once(Viewport {
					origin: [0.0, 0.0],
					depth_range: 0.0..1.0,
					dimensions: [self.width as f32, self.height as f32],
				}))
				.depth_stencil_disabled()
				.build(raster.device.clone())
				.unwrap(),
		);

		let p1_set = PersistentDescriptorSet::start(
			p1_pipeline.layout().descriptor_set_layout(0).unwrap().clone(),
		)
		.add_buffer(line_data_buf.clone())
		.unwrap()
		.add_buffer(raster.sample_data_buf.clone())
		.unwrap()
		.add_buffer(raster.ray_data_buf.clone())
		.unwrap()
		.build()
		.unwrap();

		let p1_framebuffer = Arc::new(
			Framebuffer::start(p1_render_pass.clone())
				.add(p1_out_image.clone())
				.unwrap()
				.build()
				.unwrap(),
		);

		let p2_render_pass = Arc::new(
			vulkano::single_pass_renderpass!(
				raster.device.clone(),
				attachments: {
					color: {
						load: Clear,
						store: Store,
						format: Format::R8Unorm,
						samples: 1,
					}
				},
				pass: {
					color: [color],
					depth_stencil: {}
				}
			)
			.unwrap(),
		);

		let p2_pipeline = Arc::new(
			GraphicsPipeline::start()
				.vertex_input_single_buffer::<ImtShaderVert>()
				.vertex_shader(raster.square_vs.main_entry_point(), ())
				.viewports_dynamic_scissors_irrelevant(1)
				.fragment_shader(raster.glyph_post_fs.main_entry_point(), ())
				.render_pass(Subpass::from(p2_render_pass.clone(), 0).unwrap())
				.viewports(::std::iter::once(Viewport {
					origin: [0.0, 0.0],
					depth_range: 0.0..1.0,
					dimensions: [self.width as f32, self.height as f32],
				}))
				.depth_stencil_disabled()
				.build(raster.device.clone())
				.unwrap(),
		);

		let p2_out_image = AttachmentImage::with_usage(
			raster.device.clone(),
			[self.width, self.height],
			Format::R8Unorm,
			ImageUsage {
				transfer_source: true,
				color_attachment: true,
				..vulkano::image::ImageUsage::none()
			},
		)
		.unwrap();

		let p2_framebuffer = Arc::new(
			Framebuffer::start(p2_render_pass.clone())
				.add(p2_out_image.clone())
				.unwrap()
				.build()
				.unwrap(),
		);

		let p2_set = PersistentDescriptorSet::start(
			p2_pipeline.layout().descriptor_set_layout(0).unwrap().clone(),
		)
		.add_sampled_image(p1_out_image.clone(), raster.sampler.clone())
		.unwrap()
		.build()
		.unwrap();

		let buffer_out = CpuAccessibleBuffer::from_iter(
			raster.device.clone(),
			BufferUsage::all(),
			false,
			(0..self.width * self.height).map(|_| 0u8),
		)
		.unwrap();

		AutoCommandBufferBuilder::primary_one_time_submit(
			raster.device.clone(),
			raster.queue.family(),
		)
		.unwrap()
		.begin_render_pass(p1_framebuffer.clone(), false, vec![[0.0].into()])
		.unwrap()
		.draw(
			p1_pipeline.clone(),
			&vulkano::command_buffer::DynamicState::none(),
			raster.square_buf.clone(),
			p1_set,
			(),
		)
		.unwrap()
		.end_render_pass()
		.unwrap()
		.begin_render_pass(p2_framebuffer.clone(), false, vec![[0.0].into()])
		.unwrap()
		.draw(
			p2_pipeline.clone(),
			&vulkano::command_buffer::DynamicState::none(),
			raster.square_buf.clone(),
			p2_set,
			(),
		)
		.unwrap()
		.end_render_pass()
		.unwrap()
		.copy_image_to_buffer(p2_out_image.clone(), buffer_out.clone())
		.unwrap()
		.build()
		.unwrap()
		.execute(raster.queue.clone())
		.unwrap()
		.then_signal_fence_and_flush()
		.unwrap()
		.wait(None)
		.unwrap();

		let buf_read = buffer_out.read().unwrap();

		for (y, chunk) in buf_read.chunks(self.width as usize).enumerate() {
			for (x, val) in chunk.iter().enumerate() {
				self.data[x][y] = *val as f32 / u8::max_value() as f32;
			}
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
			&ImtGeometry::Curve(ref points) =>
				self.draw_curve(&points[0], &points[1], &points[2]),
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
