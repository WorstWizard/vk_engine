#version 450

precision mediump float;
layout(location = 0) in vec3 pos;

layout(binding = 0) uniform UBO {
    mat4 model;
    mat4 view;
    mat4 proj;
} ubo;

layout(location = 0) out vec3 fragColor;

void main() {
    float shadeVariance = gl_VertexIndex / 7.0; // Range 0.0 to 1.0
    fragColor = vec3(
        1.0,
        1.0 - shadeVariance,
        1.0 - shadeVariance
    );
    gl_Position = ubo.proj * ubo.view * ubo.model * vec4(pos, 1.0);
}