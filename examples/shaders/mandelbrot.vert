#version 450

precision highp float;

layout(location = 0) in vec2 inPosition;

layout(push_constant) uniform UBlock {
    float theta;
} PushConstants;
layout(location = 0) out vec2 complexPos;

/*
vec2 positions[4] = {
    vec2(-1.0,-1.0),
    vec2( 1.0,-1.0),
    vec2(-1.0, 1.0),
    vec2( 1.0, 1.0)
};
*/

//float zoom = 0.01*cos(PushConstants.theta) + 0.0101;
float zoom = 0.1*((PushConstants.theta-1.0) * (PushConstants.theta-1.0)) + 0.001;
vec2 center = vec2(-0.55, 0.55);

vec2 complexPositions[4] = {
    vec2(center[0] - zoom, center[1] - zoom),
    vec2(center[0] + zoom, center[1] - zoom),
    vec2(center[0] - zoom, center[1] + zoom),
    vec2(center[0] + zoom, center[1] + zoom)
};

void main() {
    gl_Position = vec4(inPosition, 0.0, 1.0);
    complexPos = complexPositions[gl_VertexIndex];
}