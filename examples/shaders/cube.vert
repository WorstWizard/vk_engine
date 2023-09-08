#version 450

precision mediump float;
layout(location = 0) in vec3 pos;
layout(location = 1) in vec2 uv;

layout(binding = 0) uniform UBO {
    mat4 model;
    mat4 view;
    mat4 proj;
} ubo;

layout(location = 1) out vec2 fragUV;

void main() {
    fragUV = uv;
    gl_Position = ubo.proj * ubo.view * ubo.model * vec4(pos, 1.0);
}