#version 330 core

uniform vec3 offset;

in vec3 position;

out vec4 v_color;

void main() {
    vec4 world_pos = vec4(position + offset, 1);
    gl_Position = world_pos;

//    float local_depth = clamp(-world_pos.z / 25.0, 0, 1);
    v_color = vec4(1,1,1,1);
}