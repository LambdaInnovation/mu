#version 450

layout(set=0, binding=0) uniform texture2D u_texture;
layout(set=0, binding=1) uniform sampler u_sampler;

layout(location=0) in vec2 f_uv;
layout(location=1) in vec4 f_color;

layout(location=0) out vec4 frag_color;

void main() {
    frag_color = f_color * texture(sampler2D(u_texture, u_sampler), f_uv);
}
