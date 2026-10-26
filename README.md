# NWorld – Terrain Flyover

An endless, procedurally generated 3D landscape you fly over with the keyboard. Written in Rust with [macroquad](https://github.com/not-fl3/macroquad).

```bash
cargo run --release
```

## Controls

| Keys | Action |
| --- | --- |
| W / S or Up / Down | pitch up / down |
| A / D or Left / Right | turn left / right |
| Shift / Ctrl (or + / -) | throttle up / down |
| Space (hold) | boost x3 |
| Tab | autopilot: holds a height above the terrain (W/S change it) |
| I | invert pitch |
| [ / ] | time of day |
| N | new random world |
| R | reset position |
| F | toggle fullscreen |
| H | hide / show help |
| Esc | quit |

## How it works

- `src/noise.rs`: seeded Perlin noise, fBm and ridged multifractal.
- `src/terrain.rs`: the height field (domain-warped continents, ridged mountains, hills) and a chunk grid that streams in around the camera with 4 levels of detail. Skirts hide LOD cracks, and frustum culling skips off-screen chunks.
- `src/flight.rs`: an arcade flight model with banking, ground avoidance and a look-ahead terrain-following autopilot.
- `src/sky.rs`: the time-of-day palette, the sky shader (gradient and sun) and the terrain shader (lighting, water ripples and glints, distance fog).
- `build.rs`: generates `libimm32.a` for Rust's self-contained `windows-gnu` toolchain, whose bundled MinGW doesn't include it. It does nothing on other targets.
