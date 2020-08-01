#version 330 core

uniform sampler2D u_texture;

in vec2 f_uv;
in vec4 f_color;

out vec4 frag_color;

void main() {
    frag_color = f_color * texture(u_texture, f_uv);
}
