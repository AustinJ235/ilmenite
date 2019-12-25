use crate::ImtError;
use crate::ImtParsedGlyph;
use crate::ImtRaster;
use crate::ImtParser;
use crate::ImtGeometry;
use crate::ImtPoint;
use crate::ImtShaderVert;
use crate::shaders::glyph_base_fs;

use std::sync::Arc;
use vulkano::descriptor::descriptor_set::PersistentDescriptorSet;
use vulkano::descriptor::PipelineLayoutAbstract;
use vulkano::framebuffer::Framebuffer;
use vulkano::format::Format;
use vulkano::command_buffer::AutoCommandBufferBuilder;
use vulkano::pipeline::GraphicsPipeline;
use vulkano::framebuffer::Subpass;
use vulkano::pipeline::viewport::Viewport;
use vulkano::image::ImageUsage;
use vulkano::image::attachment::AttachmentImage;
use vulkano::buffer::cpu_access::CpuAccessibleBuffer;
use vulkano::buffer::BufferUsage;
use vulkano::command_buffer::CommandBuffer;
use vulkano::sync::GpuFuture;
use vulkano::pipeline::input_assembly::PrimitiveTopology;

#[derive(Clone)]
pub struct ImtGlyphBitmap {
	parsed: Arc<ImtParsedGlyph>,
	pub width: u32,
	pub height: u32,
	pub bearing_x: f32,
	pub bearing_y: f32,
	pub data: Vec<Vec<f32>>,
	lines: Vec<(ImtPoint, ImtPoint)>,
	min_x: f32,
	min_y: f32,
	max_x: f32,
	max_y: f32,
	scaler: f32,
}

impl ImtGlyphBitmap {
	pub fn new(parser: &ImtParser, parsed: Arc<ImtParsedGlyph>, text_height: f32) -> ImtGlyphBitmap {
		let scaler = parser.font_props.scaler * text_height;
		let ascender = parser.font_props.ascender * scaler;
		let min_x = parsed.min_x * scaler;
		let min_y = parsed.min_y * scaler;
		let max_x = parsed.max_x * scaler;
		let max_y = parsed.max_y * scaler;
		let bearing_x = min_x - 1.0;
		let bearing_y = ascender - max_y.floor() - 1.0;
		let width = (max_x.ceil() - min_x.ceil()) as u32 + 2;
		let height = (max_y.ceil() -min_y.ceil()) as u32 + 2;
		
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
			min_x,
			min_y,
			max_x,
			max_y,
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
			width: self.width as f32,
			height: self.height as f32,
		};
		
		for (pt_a, pt_b) in &self.lines {
			let i = line_data.count;
			line_data.lines[i as usize] = [
				pt_a.x - self.min_x + (self.min_x.ceil() - self.min_x) + 1.0,
				pt_a.y - self.min_y + (self.min_y.ceil() - self.min_y) + 1.0,
				pt_b.x - self.min_x + (self.min_x.ceil() - self.min_x) + 1.0,
				pt_b.y - self.min_y + (self.min_y.ceil() - self.min_y) + 1.0
			];
			line_data.count += 1;
		}
		
		let line_data_buf = CpuAccessibleBuffer::from_data(
			raster.device.clone(),
			BufferUsage::all(), // TODO: Specific Usage
			line_data
		).unwrap();
		
		let p1_out_image = AttachmentImage::with_usage(
			raster.device.clone(),
			[self.width, self.height],
			Format::R8Unorm,
			ImageUsage {
				transfer_source: true,
				color_attachment: true,
				sampled: true,
				.. vulkano::image::ImageUsage::none()
			}
		).unwrap();
		
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
			).unwrap()
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
					depth_range: 0.0 .. 1.0,
					dimensions: [self.width as f32, self.height as f32],
				}))
				.depth_stencil_disabled()
				.build(raster.device.clone()).unwrap()
		);
		
		let p1_set = PersistentDescriptorSet::start(p1_pipeline.descriptor_set_layout(0).unwrap().clone())
			.add_buffer(line_data_buf.clone()).unwrap()
			.add_buffer(raster.sample_data_buf.clone()).unwrap()
			.add_buffer(raster.ray_data_buf.clone()).unwrap()
			.build().unwrap();

		let p1_framebuffer = Arc::new(
			Framebuffer::start(p1_render_pass.clone())
				.add(p1_out_image.clone()).unwrap()
				.build().unwrap()
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
			).unwrap()
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
					depth_range: 0.0 .. 1.0,
					dimensions: [self.width as f32, self.height as f32],
				}))
				.depth_stencil_disabled()
				.build(raster.device.clone()).unwrap()
		);
		
		let p2_out_image = AttachmentImage::with_usage(
			raster.device.clone(),
			[self.width, self.height],
			Format::R8Unorm,
			ImageUsage {
				transfer_source: true,
				color_attachment: true,
				.. vulkano::image::ImageUsage::none()
			}
		).unwrap();

		let p2_framebuffer = Arc::new(
			Framebuffer::start(p2_render_pass.clone())
				.add(p2_out_image.clone()).unwrap()
				.build().unwrap()
		);
		
		let p2_set = PersistentDescriptorSet::start(p2_pipeline.descriptor_set_layout(0).unwrap().clone())
			.add_sampled_image(p1_out_image.clone(), raster.sampler.clone()).unwrap()
			.build().unwrap();
			
		let buffer_out = CpuAccessibleBuffer::from_iter(
			raster.device.clone(),
			BufferUsage::all(),
			(0 .. self.width * self.height).map(|_| 0u8)
		).unwrap();
			
		AutoCommandBufferBuilder::primary_one_time_submit(
			raster.device.clone(),
			raster.queue.family()
		).unwrap()
			.begin_render_pass(
				p1_framebuffer.clone(),
				false,
				vec![[0.0].into()]
			).unwrap()
			.draw(
				p1_pipeline.clone(),
				&vulkano::command_buffer::DynamicState::none(),
				raster.square_buf.clone(),
				p1_set,
				()
			).unwrap()
			.end_render_pass().unwrap()
			.begin_render_pass(
				p2_framebuffer.clone(),
				false,
				vec![[0.0].into()]
			).unwrap()
			.draw(
				p2_pipeline.clone(),
				&vulkano::command_buffer::DynamicState::none(),
				raster.square_buf.clone(),
				p2_set,
				()
			).unwrap()
			.end_render_pass().unwrap()
			.copy_image_to_buffer(p2_out_image.clone(), buffer_out.clone()).unwrap()
			.build().unwrap()
			.execute(raster.queue.clone()).unwrap()
			.then_signal_fence_and_flush().unwrap()
			.wait(None).unwrap();
		
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
			&ImtGeometry::Curve(ref points) => self.draw_curve(&points[0], &points[1], &points[2])
		}
	}
	
	fn draw_line(
		&mut self,
		point_a: &ImtPoint,
		point_b: &ImtPoint
	) {
		self.lines.push((
			ImtPoint {
				x: point_a.x * self.scaler,
				y: point_a.y * self.scaler
			},
			ImtPoint {
				x: point_b.x * self.scaler,
				y: point_b.y * self.scaler,
			}
		));
	}
	
	fn draw_curve(
		&mut self,
		point_a: &ImtPoint,
		point_b: &ImtPoint,
		point_c: &ImtPoint
	) {
		let mut length = 0.0;
		let mut last_point = point_a.clone();
		let mut steps = 10_usize;
		
		for s in 1..=steps {
			let t = s as f32 / steps as f32;
			let next_point = ImtPoint {
				x: ((1.0-t).powi(2)*point_a.x)+(2.0*(1.0-t)*t*point_b.x)+(t.powi(2)*point_c.x),
				y: ((1.0-t).powi(2)*point_a.y)+(2.0*(1.0-t)*t*point_b.y)+(t.powi(2)*point_c.y)
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
				x: ((1.0-t).powi(2)*point_a.x)+(2.0*(1.0-t)*t*point_b.x)+(t.powi(2)*point_c.x),
				y: ((1.0-t).powi(2)*point_a.y)+(2.0*(1.0-t)*t*point_b.y)+(t.powi(2)*point_c.y)
			};
			
			self.draw_line(&last_point, &next_point);
			last_point = next_point;
		}
	}
}
