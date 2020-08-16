#version 450

layout(set=0, binding=0) uniform mat4 u_proj;

layout(location=0) in vec2 v_pos;
layout(location=1) in vec2 v_uv;

layout(location=2) in mat4 i_world_view;
layout(location=3) in vec2 i_uv_min;
layout(location=4) in vec2 i_uv_max;
layout(location=5) in vec4 i_color;

layout(location=0) out vec2 f_uv;
layout(location=1) out vec4 f_color;

void main() {
    gl_Position = u_proj * i_world_view * vec4(v_pos, 0, 1);
    f_uv = mix(i_uv_min, i_uv_max, v_uv);
    f_color = i_color;
}
