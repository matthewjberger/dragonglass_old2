# Dragonglass

Dragonglass is a [gltf 2.0](https://khronos.org/gltf) model viewer
that uses [Vulkan](https://khronos.org/vulkan) for physically-based rendering.

## Prerequisites

* [Rust](https://www.rust-lang.org/)
* [glslang](https://github.com/KhronosGroup/glslang/releases/tag/master-tot) for shader compilation (glsl -> SPIR-V)

## Instructions

To run dragonglass, run this command in the root directory:

```
cargo run --release
```

