# nworld-view

Draws an [`nworld-map`](../nworld-map/README.md) map with macroquad. The world editor and the game both use it, so a map looks the same in both.

Most programs need only `MapView`:

```rust
let mut view = MapView::new(&world, from);        // after the window exists
let camera = MapView::camera(position, target, Vec3::Y, 1.0);
view.draw(&world, &camera, time);                 // sky, backdrop, terrain and props
```

`view_distance(height)` says how far the view reaches from a camera `height` m up, and `pick_terrain` finds the ground under the mouse. The pieces under `MapView` (`terrain_mesh`, `render_data`, `render`, `props` and the resource kinds) are public for the editor, which keeps meshes and textures in step as the map is painted.

The crate only reads the map. `build.rs` makes the `imm32` import library that Rust's bundled MinGW lacks on `windows-gnu`; it does nothing on other targets, and programs that depend on this crate get it automatically.
