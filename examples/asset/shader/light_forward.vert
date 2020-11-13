#version 450

layout (set=0, binding=0) uniform Uniforms {
    mat4 u_wvp;
    vec3 u_color;
};

layout (location=0) in vec3 pos;
layout (location=1) in vec2 uv;

layout (location=0) out vec2 v_uv;
layout (location=1) out vec4 v_color;

void main() {
    gl_Position = u_wvp * vec4(pos, 1.0);
    v_uv = uv;
    v_color = vec4(u_color, 1.0);
}
