# NWorld

A Cargo workspace of four crates:

- [`WorldGeneration`](WorldGeneration/README.md): the world editor. A window where you create, shape and save a map. It is the program you run to make maps.
- [`Game`](Game/README.md): the game. It loads its map from the `Assets` folder of its data folder, `C:\Users\nrspring\OneDrive\Secure\Projects\NWorld`, and plays on it.
- [`nworld-map`](nworld-map/README.md): the map itself, shared by the editor and the game. Terrain, water, resources and regions, the steps that build them, saving and loading, and the questions a game can ask of the ground. It draws nothing and has no graphics dependencies.
- [`nworld-view`](nworld-view/README.md): draws a map with macroquad, for the editor and the game.

```bash
cargo run --release                  # the editor
cargo run --release -p nworld-game   # the game
```

Plain `cargo run` and `cargo build` at this level use the editor. `cargo test --workspace` runs the tests of all the crates.
