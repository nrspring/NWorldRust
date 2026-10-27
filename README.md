# NWorld – World Creator

A 3D world editor written in Rust with [macroquad](https://github.com/not-fl3/macroquad). The app opens with no map. Create one with the **New map** tool in the toolbar on the left. A map is a heightmap of the size you choose, and it starts out as deep ocean. A new map opens in the top-down view with the whole map in sight; press T (or the **Back** button) to drop down to a free camera that you move through the map, and that stays put until you move it.

```bash
cargo run --release
```

In VS Code, press F5 and pick **Run NWorld (release)**.

## Tools

The toolbar on the left works like Photoshop's: click an icon to open its panel, and click it again (or press Esc) to close it. Hover over an icon to see its name.

| Tool | What it does |
| --- | --- |
| New map | Enter a width and height in meters (256–12288, rounded to multiples of 64), then click **Create map** or press Enter. Tab moves between the fields. Creating a map replaces the one that is open. |
| Continents | **Adds** continents to the open map; land that is already there is never changed, so running it again adds more. Land is green, darkening with elevation, and water is dark blue. **Land to add** is the share of the whole map that this run adds as land (exact on an empty map). **Coastline roughness** runs from smooth coasts (0) to ragged ones with bays, capes and islets (100). **Shape regularity** sets how blobby the continents are: at 100 (Regular) they are compact and rounded; toward 0 (Irregular) they are stretched out up to about 3:1 along random directions and get large bays, peninsulas and inland seas, with the far end wild but still recognisable as separate landmasses. The default of 35 is well short of the rounded end. **Edge buffer** (2–25% of the map's shorter side) is a border of open water that land never enters; coasts near it meander like any other coast rather than following its straight line. **Continents** sets how many separate landmasses this run adds; channels of water always keep them apart, and apart from land already on the map. **Seed** picks the layout, and the die button picks a random seed. Each run on a map also mixes in how many runs have come before it, so pressing the button again adds different continents without changing the seed, even after an undo; the same map, settings, seed and number of earlier runs give the same result. On a crowded map a run adds only as much as fits (at most half the free water), so it may add less than you asked for; **New map** starts over. |
| Islands | **Sprinkles** islands into the open water of the open map; like Continents, it adds to what is already there and each run is one undoable step. **Islands** is how many to place (1–200) and **Island size** is a typical island's width as a share of the map's shorter side (1–15%). **Size variety** runs from all alike to a mix of tiny and large. **Scatter** runs from grouped into archipelagos to spread evenly over the water. **Coastline roughness** is as for continents. Each island gets its own shape: some are round, some lumpy, some stretched out, some with peninsulas and bays. The **Edge buffer** is shared with the Continents tool and islands never enter it. Islands keep clear of existing land and of each other, so they never touch. If there is no room for them all, it places as many as fit and says so (for example "Added 17 of 20 islands"). **Seed** and the die button work as for continents. |
| Undo | The curved-arrow button at the bottom of the toolbar, or **Ctrl+Z**, undoes the most recent edit, restoring the map exactly as it was. The last 10 steps can be undone, each continents run being one step; older steps are forgotten. The button is dimmed when there is nothing to undo, and its tooltip says how many steps are left. **New map** is not an undoable step: it replaces the map and starts a fresh history. |

## Saving and opening maps

The three buttons at the bottom of the toolbar are, from the top, **Open** (folder), **Save** (disk) and **Undo**. They use the standard Windows file dialogs.

| Action | How |
| --- | --- |
| Save | **Ctrl+S**, or the disk button. The first time, it asks where to save; after that it saves to the same file. |
| Save as | **Ctrl+Shift+S**, or Shift-click the disk button. Always asks for a file name. |
| Open | **Ctrl+O**, or the folder button. Opens the map in the top-down view. |

Maps are saved as `.nworld` files. A file holds the map's size, its heightmap and how many times continents have been generated on it (so adding continents after reopening carries on with new variations). It is compressed, so a typical 4 km map takes well under a megabyte. The undo history and camera position are not saved. Saving writes to a temporary file and then replaces the real one, so a failed save can't damage an existing map, and opening checks the file carefully, so a damaged or foreign file gives a message instead of a crash.

The map's file name is shown next to the FPS counter, with a `*` while it has edits that aren't saved. Before **New map**, **Open** or closing the app, NWorld asks whether to save those edits (Yes saves them, No throws them away, Cancel goes back to the map). A brand-new map you haven't edited doesn't count as unsaved.

## Controls

| Keys | Action |
| --- | --- |
| W / A / S / D | move forward / left / back / right |
| E / Q (or PgUp / PgDn) | move up / down |
| Mouse wheel | zoom along the view direction |
| Right-drag or arrow keys | look around |
| Shift (hold) | move faster |
| T or the **Top-down** button | fly to a top-down view of the whole world; press again to go back |
| R | reset camera |
| Ctrl+Z | undo the last edit (up to 10 steps) |
| Ctrl+S / Ctrl+Shift+S / Ctrl+O | save / save as / open |
| H | show / hide the controls list |
| Esc | close the open panel; with no panel open, quit (asking about unsaved edits first) |

Movement speed scales with height above the surface, so it suits both close-up work and crossing the map. While you type in a field, keyboard shortcuts and camera keys are paused. The mouse wheel and right-drag don't move the camera while the pointer is over the toolbar or a panel.

The on-screen overlays are kept to a minimum so the world fills the view: the toolbar, an FPS counter, the Top-down button, and a small hint for the controls list.

## How it works

- `src/world.rs`: the world heightmap (samples 4 m apart) and its rendering. The map is drawn as a grid of 64 m chunks with 4 levels of detail, skirts to hide LOD cracks, and frustum and distance culling. The heightmap is also uploaded as a texture, so land and water are decided per pixel: coastlines stay sharp at every distance, however coarse the mesh.
- `src/camera.rs`: the free editor camera and the animated overview toggle, which frames the whole map clear of the toolbar.
- `src/continents.rs`: the continent generator. It spreads continent centers over the open water and shapes each one with simplex noise: a low-frequency, two-layer domain warp bends the outlines, large-scale noise adds lobes, peninsulas and inland seas, and fractal noise sets the coastline roughness. The shape-regularity setting stretches continents along random axes, adds noise at the scale of a whole continent, and softens the land's fall-off from the middle, so irregular shapes aren't pulled back toward a circle. Water channels between continents, around existing land (found with a distance transform) and along the edge buffer are kept free of land; a meandering keep-out line just inside the buffer stops coasts running along it straight. Continents are sized first, biggest first, then each is placed where it has the most clearance from the others, from existing land and from the buffer, so they spread over the whole map. Sea level is set at the percentile that gives the requested amount of new land, and the result is merged with the old heights keeping the higher of the two. It runs on all CPU cores and takes tens of milliseconds for a 4 km map.
- `src/storage.rs`: the `.nworld` file format: encoding, decoding with validation, and saving through a temporary file. It has unit tests for round trips, truncated, corrupt and wrong-version files, and invalid sizes.
- `src/open_map.rs`: the map being edited (world, camera, undo history, file and unsaved-changes tracking) and the save, open and "save changes?" flows.
- `src/dialogs.rs`: the native file and message dialogs, using the `rfd` crate.
- `src/toast.rs`: the short messages shown at the top of the view.
- `src/history.rs`: the undo history. Each step stores only the heightmap samples it changed, with their old heights, so steps stay small however big the map is, and any future editing tool can use it. It keeps the last 10 steps and has unit tests (`cargo test`).
- `src/islands.rs`: the island generator. It builds each island's shape first (a noise-warped, stretched blob with its own irregularity), measures how much room it really needs, then places the biggest first: archipelago centers are spread over the open water, and each island goes either near one or where it has the most clearance, depending on the scatter setting. It keeps clear of the edge buffer, existing land and other islands, and has unit tests for counts, the buffer, untouched existing land, gaps, clustering and determinism.
- `src/gen_common.rs`: pieces shared by the continent and island generators: the random number generator, the distance-to-existing-land map, and gentle seeded lowland relief that fades into the coast.
- `src/ui.rs`: the icon toolbar and its tool panels.
- `src/style.rs`: the dark UI theme: colors, rounded shapes, shadows, keycaps and fonts. Fonts load from the system at startup: Inter if it's installed, then Segoe UI, then macroquad's built-in font.
- `src/render.rs`: fixed lighting, the backdrop around the world (black at the world's edge, fading to grey farther away), and the shaders: per-pixel land and water from the surface texture with an anti-aliased coastline, terrain lighting, gentle water ripples, and fog that scales with altitude.
- `src/noise.rs`: seeded 2D simplex noise and fractal (fBm) sums of it.
- `build.rs`: generates `libimm32.a` for Rust's self-contained `windows-gnu` toolchain, whose bundled MinGW doesn't include it. It does nothing on other targets.
