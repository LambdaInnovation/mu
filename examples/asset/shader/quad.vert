#version 330 core

in vec3 position;
in vec2 uv;

out vec2 frag_uv;

void main() {
    gl_Position = vec4(position, 1);
    frag_uv = uv;
}