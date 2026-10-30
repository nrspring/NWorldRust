# NWorld – Game

The game. It loads the map from `Assets\map.nworld` in `C:\Users\nrspring\OneDrive\Secure\Projects\NWorld` (the map is made with the world editor in `WorldGeneration`, and lives outside the repository) and draws it with `nworld-view`. Pass another file as the first argument to use that instead.

```bash
cargo run --release -p nworld-game
```

| Control | |
|---|---|
| W A S D or arrows | Pan (hold Shift for faster) |
| Q / E | Turn |
| Wheel | Zoom |
| Right-drag | Look around and tilt |
| Left-drag | Grab the ground |
| Esc | Close the options, or quit |

Everything the game adds to the map, such as roads, cities and units, belongs to the game. `nworld-map` knows nothing about them. The map only answers questions about the ground.

## Options

The gear button beside the day opens the options panel. Click it again, or press Esc, to close it. While the mouse is over the button or the panel, it doesn't move the camera.

- **Show regions**: draws thin lines between the map's regions, and leaves the ground as it is. It is greyed out, with a note, for a map that has no regions. The setting lasts until the game closes; it isn't saved.

## Cities

A city belongs to a country, stands on the map, and has a population from 100 up to 100,000. Once the game has a country and a person but no city, it asks you to place the first one: move the mouse over the map, where a ring and a preview of the starting buildings show where it would go (green if it can, red if not, with the reason in the prompt), and click on dry land that is not up in the mountains. A drag still moves the map, so only a click places it. The city starts with 100 people, belongs to the first country, is reported in the newsreel, and the camera flies to it.

A city is drawn in 3D, like the trees, and its size follows its population: size goes by how many times the population has doubled, so each tenfold, 100 to 1,000 to 10,000, is the same step up. **A city stops growing in size at 50,000 people.** The population can go on up to 100,000, but the city looks the same from 50,000 up. A city grows outward from where it was placed, so the buildings it already has stay where they are. Buildings stay off the water and the mountains.

**A city never grows outside the region it was founded in.** Every building, wall to wall, has to stand in the same region as the city's middle. The regions come from the map, so how big a city can look depends on how big the map's regions are: a city of 50,000 or more is up to about 450 small buildings packed over roughly 130 m, and in a region only 150 m across it fills the region and no more. If a map has no regions, the ground is the only limit. Make the regions bigger in the editor's Regions tool for room to grow.

**The look goes from medieval to a European city of the 1800s as it grows.** A village of 100 is a few timber-framed houses with steep thatched or tiled roofs. A town has a church with a tower and spire among the houses, which keep clear of its walls, and townhouses. A city of tens of thousands has tall brick and stone blocks of four to six storeys with hipped slate roofs and chimneys in the middle, townhouses around them, and timber houses at the edge, with up to three churches. The buildings are small and close together: houses are 3 to 4 m wide and blocks 5 to 6 m, on a grid 5.5 m apart, so a city is packed close however small: even a village of 100 is a tight cluster of houses, not a scatter. Nothing is taller than a church spire. The newest, grandest buildings are in the middle.

This is all in the game (`city*.rs`); the map and the generic map view know nothing about cities. A city is saved in the day's file, and carries forward:

```json
"cities": [ { "id": 1, "country": "Aldoria", "x": -120.0, "z": 441.4, "population": 100 } ]
```

## Calendar and saved game

A year has 10 months and a month has 20 days; a day is written `year-month-day`, and the first is 1-1-1.

The saved game is in the `Saves` folder of the data folder (`C:\Users\nrspring\OneDrive\Secure\Projects\NWorld`):

- `Saves\game.json` is the main file. It holds the current day, which tells the game which day to load at startup.
- `Saves\Days\year-0001_month-01_day-01.json` is the state of one day. Every day gets a file, and they are kept forever for statistics. Names sort in date order.

With no `game.json`, the game starts a new game: the day is set to 1-1-1 and `game.json` and the first day's file are written. A file that exists but can't be read stops the game with a message and is never overwritten. All files are plain JSON with a version number, and are written through a temporary file so a crash can't leave half a file.

## Countries and the newsreel

A day's file also holds the countries, the people and that day's newsreel:

```json
"countries": [ { "name": "Aldoria" } ],
"people": [ { "name": "Mara Voss", "title": "President", "country": "Aldoria" } ],
"newsreel": [
  { "type": "country_founded", "country": "Aldoria" },
  { "type": "person_joined", "person": "Mara Voss", "title": "President", "country": "Aldoria" }
]
```

- **Countries and people** carry forward: the next day's file starts with the same ones. The first country is the player's. Each person belongs to a country, by its name.
- **The newsreel** belongs to its day alone: the next day starts with an empty one. Each item has a `type`, so statistics can count them by kind. Things like a country being founded are added to it, and it shows in the panel at the top right.

If the saved game has no country, a popup asks for the name of a new one when the game starts. Type the name and press Enter (or click **Found country**). The country is saved, and the day's newsreel reports it.

If there is a country but no people, a second popup asks for the name and title of a person, and the person is added to the first country. Tab (or the arrow keys, or a click) moves between the two fields, and Enter in the name field moves on to the title.

## Autosave

The game saves itself. Every 30 seconds, if anything in the current day has changed since it was last saved, the day's file is written, and it is written once more when the game closes (Esc or the window's close button). A day that hasn't changed is not rewritten. Founding a country is saved at once. If a save fails, a red notice under the top bar says why, and the game tries again at the next interval.
