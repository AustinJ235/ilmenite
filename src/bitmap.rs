use crate::{
	shaders::glyph_cs, ImtError, ImtGeometry, ImtParsedGlyph, ImtParser, ImtPoint, ImtRaster,
	ImtRasterOpts,
};

use std::{iter, sync::Arc};
use vulkano::{
	buffer::{cpu_access::CpuAccessibleBuffer, BufferUsage},
	command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage, PrimaryCommandBuffer},
	descriptor::descriptor_set::PersistentDescriptorSet,
	pipeline::{ComputePipeline, ComputePipelineAbstract},
	sync::GpuFuture,
};

/// Data is Linear RGBA
#[derive(Clone)]
pub struct ImtGlyphBitmap {
	parsed: Arc<ImtParsedGlyph>,
	pub width: u32,
	pub height: u32,
	pub bearing_x: f32,
	pub bearing_y: f32,
	lines: Vec<(ImtPoint, ImtPoint)>,
	scaler: f32,
	offset_x: f32,
	offset_y: f32,
	pub data: Option<Arc<Vec<f32>>>,
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
			width,
			height,
			bearing_x,
			bearing_y,
			offset_x,
			offset_y,
			data: None,
			lines: Vec::new(),
			scaler,
		}
	}

	pub(crate) fn raster_cpu(&mut self, raster: &ImtRaster) -> Result<(), ImtError> {
		let ray_count = raster.ray_count();
		let mut rays: Vec<[f32; 2]> = Vec::with_capacity(ray_count);

		for i in 0..ray_count {
			let rad = (i as f32 * (360.0 / ray_count as f32)).to_radians();
			rays.push([rad.cos(), rad.sin()]);
		}

		let sample_count = raster.sample_count();
		let mut samples: Vec<[f32; 2]> = Vec::with_capacity(sample_count);
		let w = (sample_count as f32).sqrt() as usize;

		for x in 1..=w {
			for y in 1..=w {
				samples.push([
					((x as f32 / (w as f32 + 1.0)) * 2.0) - 1.0,
					((y as f32 / (w as f32 + 1.0)) * 2.0) - 1.0,
				]);
			}
		}

		let ray_intersects = |l1p1: [f32; 2],
		                      l1p2: [f32; 2],
		                      l2p1: [f32; 2],
		                      l2p2: [f32; 2]|
		 -> Option<[f32; 2]> {
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
			let mut least_hits = -1;
			let mut ray_min_dist_sum = 0.0;

			for ray in rays.iter() {
				let mut hits = 0_isize;

				let ray_dest =
					[ray_src[0] + (ray[0] * ray_len), ray_src[1] + (ray[1] * ray_len)];

				let ray_angle = (ray[1] / ray[0]).atan();
				let mut ray_max_dist = (cell_width / 2.0) / ray_angle.cos();

				if ray_max_dist > (cell_height / 2.0) {
					ray_max_dist = (cell_height / 2.0) / (1.570796327 - ray_angle).cos();
				}

				let mut ray_min_dist = ray_max_dist;

				for line in self.lines.iter() {
					match ray_intersects(ray_src, ray_dest, [line.0.x, line.0.y], [
						line.1.x, line.1.y,
					]) {
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

				ray_min_dist_sum += ray_min_dist / ray_max_dist;

				if least_hits == -1 || hits < least_hits {
					least_hits = hits;
				}
			}

			if least_hits != -1 && least_hits % 2 != 0 {
				Some(ray_min_dist_sum / ray_count as f32)
			} else {
				None
			}
		};

		let transform_coords =
			|coords: [usize; 2], offset_i: usize, offset: [f32; 2]| -> [f32; 2] {
				let mut coords = [coords[0] as f32, coords[1] as f32 * -1.0];
				coords[0] -= self.offset_x;
				coords[1] -= self.offset_y;
				coords[0] += samples[offset_i][0];
				coords[1] += samples[offset_i][1];
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
				if let Some(fill_amt) =
					sample_filled(transform_coords(coords, i, offset), ray_len)
				{
					fill_amt_sum += fill_amt;
				}
			}

			fill_amt_sum / sample_count as f32
		};

		let mut bitmap: Vec<f32> = Vec::with_capacity((self.width * self.height * 4) as usize);
		bitmap.resize((self.width * self.height * 4) as usize, 0.0);
		let ray_len = ((self.width as f32 / self.scaler).powi(2)
			+ (self.height as f32 / self.scaler).powi(2))
		.sqrt();

		for x in 0..self.width {
			for y in 0..self.height {
				let rindex = (((y * self.width) + x) * 4) as usize;
				bitmap[rindex] = get_value([x as usize, y as usize], [1.0 / 6.0, 0.0], ray_len);
				bitmap[rindex + 1] =
					get_value([x as usize, y as usize], [3.0 / 6.0, 0.0], ray_len);
				bitmap[rindex + 2] =
					get_value([x as usize, y as usize], [5.0 / 6.0, 0.0], ray_len);
				bitmap[rindex + 3] =
					(bitmap[rindex] + bitmap[rindex + 1] + bitmap[rindex + 2]) / 3.0;
			}
		}

		self.data = Some(Arc::new(bitmap));
		Ok(())
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
				(self.width * self.height * 4) as usize,
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
					uniform_buffer: true,
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
					bounds: [
						self.parsed.min_x,
						self.parsed.max_x,
						self.parsed.min_y,
						self.parsed.max_y,
					],
					offset: [self.offset_x, self.offset_y],
					_dummy0: [0; 8],
				},
			)
			.unwrap();

		let pipeline = Arc::new(
			ComputePipeline::new(
				raster.device(),
				&raster.glyph_shader().main_entry_point(),
				&(),
				None,
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

		let mut cmd_buf = AutoCommandBufferBuilder::primary(
			raster.device(),
			raster.queue_ref().family(),
			CommandBufferUsage::OneTimeSubmit,
		)
		.unwrap();

		cmd_buf
			.dispatch([self.width, self.height, 1], pipeline, descriptor_set, (), iter::empty())
			.unwrap();

		cmd_buf
			.build()
			.unwrap()
			.execute(raster.queue())
			.unwrap()
			.then_signal_fence_and_flush()
			.unwrap()
			.wait(None)
			.unwrap();

		self.data = Some(Arc::new(bitmap_data_buf.read().unwrap().iter().cloned().collect()));
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
