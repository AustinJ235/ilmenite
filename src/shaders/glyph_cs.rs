pub mod glyph_cs {
	vulkano_shaders::shader!{
		ty: "compute",
		src: "
#version 450

layout(local_size_x = 1, local_size_y = 1, local_size_z = 1) in;

layout(set = 0, binding = 0) buffer SampleData {
	vec4 offset[];
} samples;

layout(set = 0, binding = 1) buffer RayData {
	vec4 direction[];
} rays;

layout(set = 0, binding = 2) buffer LineData {
	vec4 point[];
} lines;

layout(set = 0, binding = 4) buffer BitmapData {
	float data[];
} bitmap;

layout(set = 0, binding = 3) buffer GlyphData {
	uint samples;
	uint rays;
	uint lines;
	float scaler;
	uint width;
	uint height;
	vec4 offset;
	vec4 bounds;
} glyph;

int ccw(vec2 p0, vec2 p1, vec2 p2) {
	float dx1 = p1.x - p0.x;
	float dy1 = p1.y - p0.y;
	float dx2 = p2.x - p0.x;
	float dy2 = p2.y - p0.y;
	
	if(dx1 * dy2 > dy1 * dx2) {
		return +1;
	}
	
	if(dx1 * dy2 < dy1 * dx2) {
		return -1;
	}
	
	if(dx1 * dx2 < 0 || dy1 * dy2 < 0) {
		return -1;
	}
	
	if((dx1 * dx1) + (dy1 * dy1) < (dx2 * dx2) + (dy2 * dy2)) {
		return +1;
	}
	
	return 0;
}

bool intersect(vec2 l1p1, vec2 l1p2, vec2 l2p1, vec2 l2p2) {
	return ccw(l1p1, l1p2, l2p1) * ccw(l1p1, l1p2, l2p2) <= 0
			&& ccw(l2p1, l2p2, l1p1) * ccw(l2p1, l2p2, l1p2) <= 0;
}

bool is_filled(vec2 ray_src, float ray_len) {
	int least_hits = -1;
	
	for(uint ray_dir_i = 0; ray_dir_i < glyph.rays; ray_dir_i++) {
		vec2 ray_dest = ray_src + (rays.direction[ray_dir_i].xy * ray_len);
		int hits = 0;
		
		for(uint line_i = 0; line_i < glyph.lines; line_i ++) {
			if(intersect(ray_src, ray_dest, lines.point[line_i].xy, lines.point[line_i].zw)) {
				hits++;
			}
		}
		
		if(least_hits == -1 || hits < least_hits) {
			least_hits = hits;
		}
	}
	
	return least_hits % 2 != 0;
}

vec2 transform_coords(uint offset_i) {
	// In TTF Y is Up so flip Y
	vec2 coords = vec2(gl_GlobalInvocationID.x, -gl_GlobalInvocationID.y);
	// Apply the pixel offset for sampling
	coords += samples.offset[offset_i].xy;
	// Bearings are rounded so image doesn't sit on pixel borders
	coords += vec2(glyph.offset.x, -glyph.offset.y);
	// Convert to font units
	coords /= glyph.scaler;
	// Bearing adjustment
	coords += vec2(glyph.bounds.x, glyph.bounds.w);
	return coords;
}

void main() {
	// Set ray length to the max possible distance.
	float ray_len = sqrt(
		pow(float(glyph.width) / glyph.scaler, 2)
			+ pow(float(glyph.height) / glyph.scaler, 2)
	);
	
	uint filled = 0;
	
	for(uint i = 0; i < glyph.samples; i++) {
		if(is_filled(transform_coords(i), ray_len)) {
			filled++;
		}
	}
	
	bitmap.data[(gl_GlobalInvocationID.x * glyph.width) + gl_GlobalInvocationID.y] = sqrt(float(filled) / float(glyph.samples));
}
	"}
}

