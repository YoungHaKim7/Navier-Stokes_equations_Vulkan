#version 450

// Displays the dye field that was transported by the fluid. The dye image is
// read as a storage image (no hardware filtering of float formats assumed) and
// filtered manually with a bilinear lookup, then filmic tone-mapped so that
// thick dye saturates to white while thin wisps keep their color.

layout(set = 0, binding = 0, rgba32f) uniform readonly image2D dye;

layout(location = 0) in vec2 v_uv;
layout(location = 0) out vec4 f_color;

vec4 bilerp(vec2 p) {
    vec2 n = vec2(imageSize(dye));
    p = clamp(p, vec2(0.0), n - 1.0);
    vec2 p0 = floor(p);
    vec2 f = p - p0;
    ivec2 c0 = ivec2(p0);
    ivec2 c1 = min(c0 + ivec2(1), ivec2(n) - 1);
    vec4 a = imageLoad(dye, c0);
    vec4 b = imageLoad(dye, ivec2(c1.x, c0.y));
    vec4 c = imageLoad(dye, ivec2(c0.x, c1.y));
    vec4 d = imageLoad(dye, c1);
    return mix(mix(a, b, f.x), mix(c, d, f.x), f.y);
}

void main() {
    // Dye row 0 is the top of the window; v_uv has its origin at the bottom.
    vec2 n = vec2(imageSize(dye));
    vec2 tc = vec2(v_uv.x, 1.0 - v_uv.y);
    vec3 dye_color = bilerp(tc * n - 0.5).rgb;

    vec3 col = vec3(1.0) - exp(-dye_color * 1.8);
    vec3 background = vec3(0.012, 0.012, 0.03);
    f_color = vec4(background + col, 1.0);
}
