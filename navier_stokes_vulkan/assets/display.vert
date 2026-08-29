#version 450

// Fullscreen triangle: three vertices cover the clip-space square, no vertex
// buffer needed. Outputs uv with (0, 0) at the bottom-left of the screen.

layout(location = 0) out vec2 v_uv;

void main() {
    vec2 pos = vec2(float((gl_VertexIndex & 1) << 2) - 1.0, float((gl_VertexIndex & 2) << 1) - 1.0);
    v_uv = pos * 0.5 + 0.5;
    gl_Position = vec4(pos, 0.0, 1.0);
}
