# NWorld – World Creator

A 3D world editor written in Rust with [macroquad](https://github.com/not-fl3/macroquad). The app opens with no map. Create one with the **New map** tool in the toolbar on the left. A map is a heightmap of the size you choose, and it starts out as deep ocean. You move a free camera through it, and the camera stays put until you move it.

```bash
cargo run --release
```

In VS Code, press F5 and pick **Run NWorld (release)**.

## Tools

The toolbar on the left works like Photoshop's: click an icon to open its panel, and click it again (or press Esc) to close it. Hover over an icon to see its name.

| Tool | What it does |
| --- | --- |
| New map | Enter a width and height in meters (256–8192, rounded to multiples of 64), then click **Create map** or press Enter. Tab moves between the fields. Creating a map replaces the one that is open. |

## Controls

| Keys | Action |
| --- | --- |
| W / A / S / D | move forward / left / back / right |
| E / Q (or Space / Ctrl, PgUp / PgDn) | move up / down |
| Mouse wheel | zoom along the view direction |
| Right-drag or arrow keys | look around |
| Shift (hold) | move faster |
| T or the **Top-down** button | fly to a top-down view of the whole world; press again to go back |
| R | reset camera |
| H | show / hide the controls list |
| Esc | close the open panel; with no panel open, quit |

Movement speed scales with height above the surface, so it suits both close-up work and crossing the map. While you type in a field, keyboard shortcuts and camera keys are paused. The mouse wheel and right-drag don't move the camera while the pointer is over the toolbar or a panel.

The on-screen overlays are kept to a minimum so the world fills the view: the toolbar, an FPS counter, the Top-down button, and a small hint for the controls list.

## How it works

- `src/world.rs`: the world heightmap (samples 4 m apart) and its rendering. The map is drawn as a grid of 64 m chunks with 4 levels of detail, skirts to hide LOD cracks, and frustum and distance culling.
- `src/camera.rs`: the free editor camera and the animated overview toggle, which frames the whole map clear of the toolbar.
- `src/ui.rs`: the icon toolbar and its tool panels.
- `src/render.rs`: fixed lighting, the backdrop around the world (black at the world's edge, fading to grey farther away), and the shaders: terrain lighting, water ripples and glints, and fog that scales with altitude.
- `src/noise.rs`: seeded Perlin noise, used for color variation on land.
- `build.rs`: generates `libimm32.a` for Rust's self-contained `windows-gnu` toolchain, whose bundled MinGW doesn't include it. It does nothing on other targets.
