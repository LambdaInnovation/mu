#version 450

layout(set=0,binding=0) uniform Uniforms {
    mat4 wvp_matrix;
};

layout(location=0)      in vec3 position;
layout(location=1)      in vec2 uv;

layout(location=0)      out vec2 frag_uv;

void main() {
    gl_Position = wvp_matrix * vec4(position, 1);
    frag_uv = uv;
}