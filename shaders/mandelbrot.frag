#version 450

layout(location = 0) in vec2 complexPos;
layout(location = 0) out vec4 outColor;

vec3 colormap(float n) {
    float steps = 2.999;
    vec3 colors[3] = {
        vec3(0.0, 0.0, 0.0),
        vec3(0.0, 0.0, 1.0),
        vec3(0.8, 0.8, 1.0),
    };

    int i0 = int( floor(steps * n) );
    int i1 = int( floor(steps * n) + 1.0 );
    float t = steps*n - float(i0);

    return (1.0-t)*colors[i0] + t*colors[i1];
}

void main() {
    int max_iter = 300;
    vec2 c = complexPos;
    vec2 z = vec2(0.0,0.0);

    int i = 0;
    while (z[0]*z[0] + z[1]*z[1] <= 4 && i < max_iter) {
        float tmp_r = z[0];
        z[0] = z[0]*z[0] - z[1]*z[1] + c[0];
        z[1] = 2*tmp_r*z[1] + c[1];
        i = i+1;
    }

    float gradient = float(i) / float(max_iter); //Interval [0, 1[
    outColor = vec4(colormap(gradient), 1.0);
}