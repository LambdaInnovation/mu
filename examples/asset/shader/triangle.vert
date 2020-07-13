#version 330 core

//uniform mat4 wvp_matrix;

in vec3 position;

out vec4 v_color;

void main() {
    vec4 world_pos = vec4(position, 1);
    gl_Position = world_pos;

//    float local_depth = clamp(-world_pos.z / 25.0, 0, 1);
    v_color = vec4(1,1,1,1);
}