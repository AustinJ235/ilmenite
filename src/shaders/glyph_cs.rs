pub mod glyph_cs {
	vulkano_shaders::shader!{
		ty: "compute",
		vulkan_version: "1.1",
		spirv_version: "1.5",
		src: "
#version 450

layout(local_size_x = 1, local_size_y = 1, local_size_z = 1) in;

layout(set = 0, binding = 0) readonly uniform Common {
	vec4 samples_and_rays[25];
	uint sample_count;
	uint ray_count;
} com;

layout(set = 0, binding = 1) readonly uniform Glyph {
	float scaler;
	uint width;
	uint height;
	uint line_count;
	vec4 bounds;
	vec2 offset;
} glyph;

layout(set = 0, binding = 2) writeonly uniform image2D bitmap;

layout(set = 0, binding = 3) readonly buffer Line {
	vec4 line[];
} lines;

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

bool sample_filled(vec2 ray_src, float ray_len, out float fill_amt) {
	vec2 intersect_point = vec2(0.0);
	int rays_filled = 0;
	float ray_fill_amt = 0.0;
	float cell_height = (glyph.scaler / sqrt(com.sample_count));
	float cell_width = cell_height / 3.0;
	
	for(uint ray_dir_i = 0; ray_dir_i < com.ray_count; ray_dir_i++) {
		int hits = 0;
		vec2 ray_dest = ray_src + (com.samples_and_rays[ray_dir_i].zw * ray_len);
		float ray_angle = atan(com.samples_and_rays[ray_dir_i].w / com.samples_and_rays[ray_dir_i].z);
		float ray_max_dist = (cell_width / 2.0) / cos(ray_angle);

		if(ray_max_dist > (cell_height / 2.0)) {
			ray_max_dist = (cell_height / 2.0) / cos(1.570796327 - ray_angle);
		}
		
		float ray_min_dist = ray_max_dist;
		
		for(uint line_i = 0; line_i < glyph.line_count; line_i ++) {
			if(ray_intersects(ray_src, ray_dest, lines.line[line_i].xy, lines.line[line_i].zw, intersect_point)) {
				float dist = distance(ray_src, intersect_point);
				
				if(dist < ray_min_dist) {
					ray_min_dist = dist;
				}
				
				hits++;
			}
		}

		if(hits % 2 != 0) {
			rays_filled++;
			ray_fill_amt += ray_min_dist / ray_max_dist;
		}
	}

	if(rays_filled >= com.ray_count / 2) {
		fill_amt = ray_fill_amt / float(rays_filled);
		return true;
	} else {
		return false;
	}
}

vec2 transform_coords(uint offset_i, vec2 offset) {
	vec2 coords = vec2(float(gl_GlobalInvocationID.x), float(gl_GlobalInvocationID.y) * -1.0);
	coords -= glyph.offset;
	// Apply the pixel offset for sampling
	coords += com.samples_and_rays[offset_i].xy;
	coords += offset;
	// Convert to font units
	coords /= glyph.scaler;
	// Bearing adjustment
	coords += vec2(glyph.bounds.x, glyph.bounds.w);
	return coords;
}

float gain( float x, float k ) {
  x = clamp(x, 0.0, 1.0);
  float s = sign(x-0.5);
  float o = (1.0+s)/2.0;
  return o - 0.5*s*pow(2.0*(o-s*x),k);
}

float get_value(vec2 offset, float ray_len) {
	float fill_amt = 0.0;
	float fill_amt_sum = 0.0;
	
	for(uint i = 0; i < com.sample_count; i++) {
		if(sample_filled(transform_coords(i, offset), ray_len, fill_amt)) {
			fill_amt_sum += fill_amt;
		}
	}
	
	float value = fill_amt_sum / float(com.sample_count);

	if(value < 0.02) {
		return 0.0;
	} else {
		return gain(value + 0.1, 2.5);
	}
}

void main() {
	float ray_len = sqrt(
		pow(float(glyph.width) / glyph.scaler, 2)
			+ pow(float(glyph.height) / glyph.scaler, 2)
	);
	
	uint rindex = ((gl_GlobalInvocationID.y * glyph.width) + gl_GlobalInvocationID.x) * 4;
	float left = get_value(vec2(-1.0 / 6.0, 0.0), ray_len);
	float r = get_value(vec2(1.0 / 6.0, 0.0), ray_len);
	float g = get_value(vec2(3.0 / 6.0, 0.0), ray_len);
	float b = get_value(vec2(5.0 / 6.0, 0.0), ray_len);
	float right = get_value(vec2(7.0 / 6.0, 0.0), ray_len);

	vec3 color = vec3(
		(left * (1.0 / 3.0)) + (r * (1.0 / 3.0)) + (g * (1.0 / 3.0)),
		(r * (1.0 / 3.0)) + (g * (1.0 / 3.0)) + (b * (1.0 / 3.0)),
		(g * (1.0 / 3.0)) + (b * (1.0 / 3.0)) + (right * (1.0 / 3.0))
	);

	float alpha = max(color.r, max(color.g, color.b));
	imageStore(bitmap, ivec2(gl_GlobalInvocationID.x, gl_GlobalInvocationID.y), vec4(color, alpha));
}
	"}
}

