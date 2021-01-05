# Hyperbezier to bezier comparison

This contains a simple [Druid] application (intended to be compiled to wasm)
that overlays a hyperbezier on a normal cubic Bézier, with the same control
points manipulating both curves.


to build:

- install `wasm-pack`
- run `wasm-pack build --target web --dev` (or `--release`)
- run some web server from the directory root
