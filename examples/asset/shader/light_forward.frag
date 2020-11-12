#version 450

layout(set=0, binding=1) uniform texture2D tex;
layout(set=0, binding=2) uniform sampler smp;

layout(location=0) in vec2 v_uv;

layout(location=0) out vec4 frag_color;

void main() {
    frag_color = texture(sampler2D(tex, smp), v_uv);
//    frag_color = vec4(1., 1., 1., 1.);
}
