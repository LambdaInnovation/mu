#version 450

layout(set=0,binding=0) uniform Uniforms {
    vec3 offset;
};

layout(location=0) in vec3 position;

layout(location=0) out vec4 v_color;

float map(float x) {
    return (x + 1) * 0.5;
}

void main() {
    vec4 world_pos = vec4(position + offset, 1);
    gl_Position = world_pos;

//    float local_depth = clamp(-world_pos.z / 25.0, 0, 1);
    v_color = vec4(map(world_pos.x),map(world_pos.y),map(world_pos.z),1);
}