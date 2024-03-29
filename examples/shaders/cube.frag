#version 450

precision mediump float;
layout(location = 1) in vec2 fragUV;

layout(binding = 1) uniform sampler2D texSampler;

layout(location = 0) out vec4 outColor;

void main() {
    outColor = texture(texSampler, fragUV);
}