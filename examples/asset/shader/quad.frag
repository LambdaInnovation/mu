#version 330 core

uniform sampler2D tex;

in vec2 frag_uv;
out vec4 fragColor;

void main() {
    fragColor = texture(tex, frag_uv);
}
