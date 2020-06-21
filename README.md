# Dragonglass

Dragonglass is a [gltf 2.0](https://khronos.org/gltf) model viewer written in Rust.

## Development Prerequisites

* [Rust](https://www.rust-lang.org/)
* [glslang](https://github.com/KhronosGroup/glslang/releases/tag/master-tot) for shader compilation (glsl -> SPIR-V)

## Instructions

To run dragonglass, run this command in the root directory:

```
cargo run --release
```

## Features

- [] Physically Based Rendering
- [] Depth of Field
- [] Chromatic Aberration
- [] Film Grain
- [] Bloom
- [] Shadow Mapping
- [] Cascaded Shadow Mapping
- [] Omnidirectional Shadow Mapping
- [] Motion Blur
- [] Deferred Rendering Pipeline
- [] Forward+ Rendering Pipeline
- [] Screen Space Reflections
- [] Screenshots

### Rendering Backends

- [] Vulkan (default)
- [] OpenGL
- [] DirectX 12
