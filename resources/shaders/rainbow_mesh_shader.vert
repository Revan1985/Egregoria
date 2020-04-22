#version 450

layout(location=0) in vec3 in_pos;
layout(location=1) in vec4 in_color;

layout(location=0) out vec4 out_color;
layout(location=1) out vec3 out_pos;

void main() {
    out_color = in_color;
    gl_Position = vec4(in_pos, 1.0);
    out_pos = in_pos;
}