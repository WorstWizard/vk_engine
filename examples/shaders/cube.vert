#version 450

precision mediump float;
layout(location = 0) in vec3 pos;

layout(binding = 0) uniform UBO {
    mat4 model;
    mat4 view;
    mat4 proj;
} ubo;

// layout(push_constant) uniform UBlock {
//     float theta;
// } PushConstants;

void main() {
    // vec3 translate = vec3(0.0,0.0,-5.0);
    // vec3 newPos = pos + translate;

    gl_Position = ubo.proj * ubo.view * ubo.model * vec4(pos, 1.0);
}