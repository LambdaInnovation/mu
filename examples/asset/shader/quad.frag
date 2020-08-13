#version 450

layout(set=0,binding=1) uniform sampler2D tex;

layout(location=0) in vec2 frag_uv;
layout(location=0) out vec4 fragColor;

void main() {
    fragColor = texture(tex, frag_uv);
}
