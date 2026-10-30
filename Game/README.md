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

- **Show regions**: tints the ground by the region it is in and draws the edges between regions. It is greyed out, with a note, for a map that has no regions. The setting lasts until the game closes; it isn't saved.

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
