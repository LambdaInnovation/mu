#version 330 core

in vec2 v_pos;
in vec2 v_uv;

in mat4 i_wvp;
in vec2 i_uv_min;
in vec2 i_uv_max;
in vec4 i_color;

out vec2 f_uv;
out vec4 f_color;

void main() {
    gl_Position = i_wvp * vec4(v_pos, 0, 1);
    f_uv = mix(i_uv_min, i_uv_max, v_uv);
    f_color = i_color;
}