pub mod glyph_cs {
	vulkano_shaders::shader!{
		ty: "compute",
		src: "
#version 450

layout(local_size_x = 1, local_size_y = 1, local_size_z = 1) in;

layout(set = 0, binding = 0) buffer SampleData {
	highp vec4 offset[];
} samples;

layout(set = 0, binding = 1) buffer RayData {
	highp vec4 direction[];
} rays;

layout(set = 0, binding = 2) buffer LineData {
	highp vec4 point[];
} lines;

layout(set = 0, binding = 3) buffer BitmapData {
	highp float data[];
} bitmap;

layout(set = 0, binding = 4) buffer GlyphData {
	uint samples;
	uint rays;
	uint lines;
	highp float scaler;
	uint width;
	uint height;
	highp vec4 bounds;
	highp vec2 offset;
} glyph;

bool ray_intersects(highp vec2 l1p1, highp vec2 l1p2, highp vec2 l2p1, highp vec2 l2p2, out highp vec2 point) {
	highp vec2 r = l1p2 - l1p1;
	highp vec2 s = l2p2 - l2p1;
	highp float det = r.x * s.y - r.y * s.x;
	highp float u = ((l2p1.x - l1p1.x) * r.y - (l2p1.y - l1p1.y) * r.x) / det;
	highp float t = ((l2p1.x - l1p1.x) * s.y - (l2p1.y - l1p1.y) * s.x) / det;
	
	if ((t >= 0. && t <= 1.) && (u >= 0. && u <= 1.)) {
		point = l1p1 + r * t;
		return true;
	} else {
		return false;
	}
}

bool sample_filled(highp vec2 ray_src, highp float ray_len, out highp float fill_amt) {
	int least_hits = -1;
	bool intersects = false;
	highp float ray_min_dist_sum = 0.0;
	highp vec2 intersect_point = vec2(0.0);
	
	for(uint ray_dir_i = 0; ray_dir_i < glyph.rays; ray_dir_i++) {
		int hits = 0;
		highp vec2 ray_dest = ray_src + (rays.direction[ray_dir_i].xy * ray_len);
		highp float cell_height = (glyph.scaler / sqrt(glyph.samples));
		highp float cell_width = cell_height / 3.0;
		highp float ray_angle = atan(rays.direction[ray_dir_i].y / rays.direction[ray_dir_i].x);
		highp float ray_max_dist = (cell_width / 2.0) / cos(ray_angle);

		if(ray_max_dist > (cell_height / 2.0)) {
			ray_max_dist = (cell_height / 2.0) / cos(1.570796327 - ray_angle);
		}
		
		highp float ray_min_dist = ray_max_dist;
		
		for(uint line_i = 0; line_i < glyph.lines; line_i ++) {
			if(ray_intersects(ray_src, ray_dest, lines.point[line_i].xy, lines.point[line_i].zw, intersect_point)) {
				highp float dist = distance(ray_src, intersect_point);
				
				if(dist < ray_min_dist) {
					ray_min_dist = dist;
				}
				
				hits++;
			}
		}
		
		ray_min_dist_sum += ray_min_dist / ray_max_dist;
		
		if(least_hits == -1 || hits < least_hits) {
			least_hits = hits;
		}
	}
	
	fill_amt = ray_min_dist_sum / float(glyph.rays);
	return least_hits % 2 != 0;
}

highp vec2 transform_coords(uint offset_i, vec2 offset) {
	highp vec2 coords = vec2(float(gl_GlobalInvocationID.x), float(gl_GlobalInvocationID.y) * -1.0);
	coords -= glyph.offset;
	// Apply the pixel offset for sampling
	coords += samples.offset[offset_i].xy;
	coords += offset;
	// Convert to font units
	coords /= glyph.scaler;
	// Bearing adjustment
	coords += vec2(glyph.bounds.x, glyph.bounds.w);
	return coords;
}

highp float get_value(highp vec2 offset, highp float ray_len) {
	highp float fill_amt = 0.0;
	highp float fill_amt_sum = 0.0;
	
	for(uint i = 0; i < glyph.samples; i++) {
		if(sample_filled(transform_coords(i, offset), ray_len, fill_amt)) {
			fill_amt_sum += fill_amt;
		}
	}
	
	return pow(fill_amt_sum / float(glyph.samples), 1.2);
}

void main() {
	highp float ray_len = sqrt(
		pow(float(glyph.width) / glyph.scaler, 2)
			+ pow(float(glyph.height) / glyph.scaler, 2)
	);
	
	uint rindex = ((gl_GlobalInvocationID.y * glyph.width) + gl_GlobalInvocationID.x) * 4;
	bitmap.data[rindex] = get_value(vec2(1.0 / 6.0, 0.0), ray_len);
	bitmap.data[rindex + 1] = get_value(vec2(3.0 / 6.0, 0.0), ray_len);
	bitmap.data[rindex + 2] = get_value(vec2(5.0 / 6.0, 0.0), ray_len);
	bitmap.data[rindex + 3] = (bitmap.data[rindex] + bitmap.data[rindex + 1] + bitmap.data[rindex + 2]) / 3.0;
}
	"}
}

