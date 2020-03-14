#[derive(Clone, Debug, PartialEq)]
pub enum ImtGeometry {
	Line([ImtPoint; 2]),
	Curve([ImtPoint; 3]),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ImtPosition {
	pub x: f32,
	pub y: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ImtPoint {
	pub x: f32,
	pub y: f32,
}

impl ImtPoint {
	pub fn lerp(&self, t: f32, other: &Self) -> Self {
		ImtPoint {
			x: self.x + ((other.x - self.x) * t),
			y: self.y + ((other.y - self.y) * t),
		}
	}

	pub fn dist(&self, other: &Self) -> f32 {
		((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
	}
}

#[derive(Default, Copy, Clone)]
pub(crate) struct ImtShaderVert {
	pub position: [f32; 2],
}

vulkano::impl_vertex!(ImtShaderVert, position);
