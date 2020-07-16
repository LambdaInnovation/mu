#version 330 core

uniform mat4 wvp_matrix;

in vec3 position;
in vec2 uv;

out vec2 frag_uv;

void main() {
    gl_Position = wvp_matrix * vec4(position, 1);
    frag_uv = uv;
}