#version 450

layout(location=0) in vec2 v_pos;
layout(location=1) in vec2 v_uv;

layout(location=2) in vec4 i_wvp_c0;
layout(location=3) in vec4 i_wvp_c1;
layout(location=4) in vec4 i_wvp_c2;
layout(location=5) in vec4 i_wvp_c3;
layout(location=6) in vec2 i_uv_min;
layout(location=7) in vec2 i_uv_max;
layout(location=8) in vec4 i_color;

layout(location=0) out vec2 f_uv;
layout(location=1) out vec4 f_color;

void main() {
    mat4 wvp = mat4(i_wvp_c0, i_wvp_c1, i_wvp_c2, i_wvp_c3);
    gl_Position = wvp * vec4(v_pos, 0, 1);
    f_uv = mix(i_uv_min, i_uv_max, v_uv);
    f_color = i_color;
}