#version 450

layout(set=0,binding=1) uniform texture2D tex;
layout(set=0,binding=2) uniform sampler smp;

layout(location=0) in vec2 frag_uv;
layout(location=0) out vec4 fragColor;

void main() {
    fragColor = texture(sampler2D(tex, smp), frag_uv);
}
