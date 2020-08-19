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

layout(set = 0, binding = 3) buffer BitmapData {
	float data[];
} bitmap;

layout(set = 0, binding = 4) buffer GlyphData {
	uint samples;
	uint rays;
	uint lines;
	float scaler;
	uint width;
	uint height;
	vec4 bounds;
	vec2 offset;
} glyph;

bool ray_intersects(vec2 l1p1, vec2 l1p2, vec2 l2p1, vec2 l2p2, out vec2 point) {
	vec2 r = l1p2 - l1p1;
	vec2 s = l2p2 - l2p1;
	float det = r.x * s.y - r.y * s.x;
	float u = ((l2p1.x - l1p1.x) * r.y - (l2p1.y - l1p1.y) * r.x) / det;
	float t = ((l2p1.x - l1p1.x) * s.y - (l2p1.y - l1p1.y) * s.x) / det;
	
	if ((t >= 0. && t <= 1.) && (u >= 0. && u <= 1.)) {
		point = l1p1 + r * t;
		return true;
	} else {
		return false;
	}
}

vec2 pixel_to_glyph_space(vec2 point) {
	point.y *= -1.0;
	point -= glyph.offset;
	point /= glyph.scaler;
	point += glyph.bounds.xw;
	return point;
}

float get_cell_value(float w, vec2 point) {
	float outer_dim = float(max(glyph.width, glyph.height)) * 2.0;
	vec2 tl_corner = pixel_to_glyph_space(point + vec2(-outer_dim, outer_dim));
	vec2 bl_corner = pixel_to_glyph_space(point + vec2(-outer_dim, -outer_dim));
	vec2 tr_corner = pixel_to_glyph_space(point + vec2(outer_dim, outer_dim));
	vec2 br_corner = pixel_to_glyph_space(point + vec2(outer_dim, -outer_dim));
	point = pixel_to_glyph_space(point);
	int tl_hits = 0;
	int bl_hits = 0;
	int tr_hits = 0;
	int br_hits = 0;
	float pixel_width = w / glyph.scaler;
	float max_ray_dist = sqrt(2 * pow(pixel_width / 2.0, 2.0));
	float tl_min_dist = max_ray_dist;
	float bl_min_dist = max_ray_dist;
	float tr_min_dist = max_ray_dist;
	float br_min_dist = max_ray_dist;
	vec2 intersect_point = vec2(0.0);

	for (uint line_i = 0; line_i < glyph.lines; line_i++) {
		if(ray_intersects(tl_corner, point, lines.point[line_i].xy, lines.point[line_i].zw, intersect_point)) {
			tl_min_dist = min(distance(intersect_point, point), tl_min_dist);
			tl_hits++;
		}

		if(ray_intersects(bl_corner, point, lines.point[line_i].xy, lines.point[line_i].zw, intersect_point)) {
			bl_min_dist = min(distance(intersect_point, point), bl_min_dist);
			bl_hits++;
		}

		if(ray_intersects(tr_corner, point, lines.point[line_i].xy, lines.point[line_i].zw, intersect_point)) {
			tr_min_dist = min(distance(intersect_point, point), tr_min_dist);
			tr_hits++;
		}

		if(ray_intersects(br_corner, point, lines.point[line_i].xy, lines.point[line_i].zw, intersect_point)) {
			br_min_dist = min(distance(intersect_point, point), br_min_dist);
			br_hits++;
		}
	}

	int hits = max(max(tl_hits, bl_hits), max(tr_hits, br_hits));
	float value = 0.0;

	if(hits % 2 != 0) {
		vec2 tl_point = (tl_min_dist / max_ray_dist) * vec2(-0.5, 0.5);
		vec2 tr_point = (tr_min_dist / max_ray_dist) * vec2(0.5, 0.5);
		vec2 bl_point = (bl_min_dist / max_ray_dist) * vec2(-0.5, -0.5);
		vec2 br_point = (br_min_dist / max_ray_dist) * vec2(0.5, -0.5);
		float top_length = distance(tl_point, tr_point);
		float bottom_length = distance(bl_point, br_point);
		float left_length = distance(tl_point, bl_point);
		float right_length = distance(tr_point, br_point);
		float diag_length = distance(tl_point, br_point);
		float bl_angle = acos((pow(left_length, 2.0) + pow(bottom_length, 2.0) - pow(diag_length, 2.0)) / (2.0 * left_length * bottom_length));
		float tr_angle = acos((pow(right_length, 2.0) + pow(top_length, 2.0) - pow(diag_length, 2.0)) / (2.0 * right_length * top_length));
		value = (0.5 * left_length * bottom_length * sin(bl_angle)) + (0.5 * top_length * right_length * sin(tr_angle));
	}

	return value;
}

void main() {
	vec2 point = vec2(float(gl_GlobalInvocationID.x), float(gl_GlobalInvocationID.y));
	vec3 value = vec3(0.0);

	for (uint sample_i = 0; sample_i < glyph.samples; sample_i++) {
		if(sample_i < glyph.samples / 3) { // R
			value.r += get_cell_value(
				samples.offset[sample_i].z,
				point + samples.offset[sample_i].xy
			) * samples.offset[sample_i].w;
		} else if(sample_i < (glyph.samples / 3) * 2) { // G
			value.g += get_cell_value(
				samples.offset[sample_i].z,
				point + samples.offset[sample_i].xy
			) * samples.offset[sample_i].w;
		} else { // B
			value.b += get_cell_value(
				samples.offset[sample_i].z,
				point + samples.offset[sample_i].xy
			) * samples.offset[sample_i].w;
		}
	}

	float average = (value.r + value.g + value.b) / 3.0;
	uint index = ((gl_GlobalInvocationID.y * glyph.width) + gl_GlobalInvocationID.x) * 4;
	bitmap.data[index] = mix(average, value.r, 0.2);
	bitmap.data[index + 1] = mix(average, value.g, 0.2);
	bitmap.data[index + 2] = mix(average, value.b, 0.2);
	bitmap.data[index + 3] = average;
}
	"}
}

