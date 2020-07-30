#version 450

layout(location = 0) in vec2 inUV;

layout(binding = 0) uniform sampler2D color;

layout(location = 0) out vec4 outColor;

void main() {
  outColor = texture(color, inUV);
}
