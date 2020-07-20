#version 330 core

uniform mat4 u_proj;

layout(location = 0) in vec2 v_pos;
layout(location = 1) in vec2 v_uv;

layout(location = 2) in mat4 i_world_view;
layout(location = 2) in vec4 i_world_view0;
layout(location = 3) in vec4 i_world_view1;
layout(location = 4) in vec4 i_world_view2;
layout(location = 5) in vec4 i_world_view3;
layout(location = 6) in vec2 i_uv_min;
layout(location = 7) in vec2 i_uv_max;

out vec2 v_uv;

void main() {
    gl_Position = vec4(vec3(0.0), 1.0);
}
