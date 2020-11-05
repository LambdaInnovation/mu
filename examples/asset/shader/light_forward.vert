#version 450

layout (set=0, binding=0) uniform Uniforms {
    mat4 u_wvp;
};

layout (location=0) in vec3 pos;
layout (location=1) in vec2 uv;

layout (location=0) out vec2 v_uv;

void main() {
    gl_Position = u_wvp * vec4(pos, 1.0);
    v_uv = uv;
}
