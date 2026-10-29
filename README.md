# NWorld

A Cargo workspace with two crates:

- [`WorldGeneration`](WorldGeneration/README.md): the world editor. A window where you create, shape and save a map, and look at it. It is the program you run.
- [`nworld-map`](nworld-map/README.md): the map itself, shared by the editor and by other applications. Terrain, water, resources and regions, the steps that build them, saving and loading, and the questions a game can ask of the ground. It draws nothing and has no graphics dependencies.

```bash
cargo run --release
```

Plain `cargo run` and `cargo build` at this level use the editor. `cargo test --workspace` runs the tests of both crates.
