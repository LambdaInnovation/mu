#version 330 core

uniform sampler2D u_texture;

in vec2 f_uv;

out vec4 frag_color;

void main() {
    frag_color = texture(u_texture, f_uv);
}
