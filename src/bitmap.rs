use crate::{
	shaders::glyph_cs, ImtError, ImtGeometry, ImtParsedGlyph, ImtParser, ImtPoint, ImtRaster,
};

use crate::vulkano::descriptor::PipelineLayoutAbstract;
use std::sync::Arc;
use vulkano::{
	buffer::{cpu_access::CpuAccessibleBuffer, BufferUsage},
	command_buffer::{AutoCommandBufferBuilder, CommandBuffer},
	descriptor::descriptor_set::PersistentDescriptorSet,
	pipeline::ComputePipeline,
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
		parser: &ImtParser,
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

		let mut line_data = Vec::with_capacity(self.lines.len());

		for (pt_a, pt_b) in &self.lines {
			line_data.push([pt_a.x, pt_a.y, pt_b.x, pt_b.y]);
		}

		let line_data_buf: Arc<CpuAccessibleBuffer<[[f32; 4]]>> =
			CpuAccessibleBuffer::from_iter(
				raster.device(),
				BufferUsage {
					storage_buffer: true,
					uniform_buffer: true,
					..BufferUsage::none()
				},
				false,
				line_data.into_iter(),
			)
			.unwrap();

		let bitmap_data_buf: Arc<CpuAccessibleBuffer<[f32]>> = unsafe {
			CpuAccessibleBuffer::uninitialized_array(
				raster.device(),
				(self.width * self.height) as usize,
				BufferUsage::all(),
				true,
			)
			.unwrap()
		};

		let glyph_data_buf: Arc<CpuAccessibleBuffer<glyph_cs::ty::GlyphData>> =
			CpuAccessibleBuffer::from_data(
				raster.device(),
				BufferUsage {
					storage_buffer: true,
					..BufferUsage::none()
				},
				false,
				glyph_cs::ty::GlyphData {
					samples: raster.sample_count() as u32,
					rays: raster.ray_count() as u32,
					lines: self.lines.len() as u32,
					scaler: self.scaler,
					width: self.width,
					height: self.height,
					offset: [self.pixel_align_offset_x, self.pixel_align_offset_y, 0.0, 0.0],
					bounds: [
						self.parsed.min_x,
						self.parsed.max_x,
						self.parsed.min_y,
						self.parsed.max_y,
					],
					_dummy0: [0; 8],
				},
			)
			.unwrap();

		let pipeline = Arc::new(
			ComputePipeline::new(
				raster.device(),
				&raster.glyph_shader().main_entry_point(),
				&(),
			)
			.unwrap(),
		);

		let descriptor_set = PersistentDescriptorSet::start(
			pipeline.layout().descriptor_set_layout(0).unwrap().clone(),
		)
		.add_buffer(raster.sample_data_buf())
		.unwrap()
		.add_buffer(raster.ray_data_buf())
		.unwrap()
		.add_buffer(line_data_buf)
		.unwrap()
		.add_buffer(bitmap_data_buf.clone())
		.unwrap()
		.add_buffer(glyph_data_buf.clone())
		.unwrap()
		.build()
		.unwrap();

		AutoCommandBufferBuilder::primary_one_time_submit(
			raster.device(),
			raster.queue_ref().family(),
		)
		.unwrap()
		.dispatch([self.width, self.height, 1], pipeline, descriptor_set, ())
		.unwrap()
		.build()
		.unwrap()
		.execute(raster.queue())
		.unwrap()
		.then_signal_fence_and_flush()
		.unwrap()
		.wait(None)
		.unwrap();

		let buf_read = bitmap_data_buf.read().unwrap();

		for (y, chunk) in buf_read.chunks(self.width as usize).enumerate() {
			for (x, val) in chunk.iter().enumerate() {
				self.data[x][y] = *val;
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
