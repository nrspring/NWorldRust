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

- **Show regions**: draws thin lines between the map's regions, and leaves the ground as it is. It is **on** when the game starts. It is greyed out, with a note, for a map that has no regions. Turning it off lasts until the game closes; it isn't saved, so it is on again next time.

## Cities

A city has a name, belongs to a country, stands on the map, and has a population from 100 up to 100,000. Once the game has a country and a person but no city, it asks you to place the first one: move the mouse over the map, where a ring and a preview of the starting buildings show where it would go (green if it can, red if not, with the reason in the prompt), and click on dry land that is not up in the mountains. A drag still moves the map, so only a click places it. A popup then asks for the city's name (Esc gives up and goes back to placing it). The city starts with 100 people, belongs to the first country, is reported in the newsreel, and the camera flies to it. Two cities can't have the same name.

**Hover over a city** to see a small panel with its name, its country and its population (with commas, like 12,345). A city counts as under the mouse when the mouse is over its buildings or just beside them, with a little extra room so a far-off city is not hard to point at.

A city is drawn in 3D, like the trees, and its size follows its population: size goes by how many times the population has doubled, so each tenfold, 100 to 1,000 to 10,000, is the same step up. **A city stops growing in size at 50,000 people.** The population can go on up to 100,000, but the city looks the same from 50,000 up. A city grows outward from where it was placed, so the buildings it already has stay where they are. Buildings stay off the water and the mountains.

**A city never grows outside the region it was founded in.** Every building, wall to wall, has to stand in the same region as the city's middle. The regions come from the map, so how big a city can look depends on how big the map's regions are: a city of 50,000 or more is up to about 450 small buildings packed over roughly 130 m, and in a region only 150 m across it fills the region and no more. If a map has no regions, the ground is the only limit. Make the regions bigger in the editor's Regions tool for room to grow.

**The look goes from medieval to a European city of the 1800s as it grows.** A village of 100 is a few timber-framed houses with steep thatched or tiled roofs. A town has a church with a tower and spire among the houses, which keep clear of its walls, and townhouses. A city of tens of thousands has tall brick and stone blocks of four to six storeys with hipped slate roofs and chimneys in the middle, townhouses around them, and timber houses at the edge, with up to three churches. The buildings are small and close together: houses are 3 to 4 m wide and blocks 5 to 6 m, on a grid 5.5 m apart, so a city is packed close however small: even a village of 100 is a tight cluster of houses, not a scatter. Nothing is taller than a church spire. The newest, grandest buildings are in the middle.

This is all in the game (`city*.rs`); the map and the generic map view know nothing about cities. A city is saved in the day's file, and carries forward:

```json
"cities": [ { "id": "47067f7a-fdac-4040-b652-873cebb0c53d", "country": "3f2b8c1e-5a47-4d0e-9b6a-7c1d2e8f4a90", "name": "Port Aldor",
              "x": -120.0, "z": 441.4, "population": 100 } ]
```

## Calendar and saved game

A year has 10 months and a month has 20 days; a day is written `year-month-day`, and the first is 1-1-1.

The saved game is in the `Saves` folder of the data folder (`C:\Users\nrspring\OneDrive\Secure\Projects\NWorld`):

- `Saves\game.json` is the main file. It holds the current day, which tells the game which day to load at startup.
- `Saves\Days\year-0001_month-01_day-01.json` is the state of one day. Every day gets a file, and they are kept forever for statistics. Names sort in date order.

With no `game.json`, the game starts a new game: the day is set to 1-1-1 and `game.json` and the first day's file are written. A file that exists but can't be read stops the game with a message and is never overwritten. All files are plain JSON with a version number (a day's file is version 2, the one that refers to everything by GUID; earlier day files named things by name and can't be loaded), and are written through a temporary file so a crash can't leave half a file.

## Countries and the newsreel

A day's file also holds the countries, the people and that day's newsreel:

```json
"title_types": [ { "id": "5c76242a-e7a0-4751-aa52-0d295a219590", "name": "Head of State" } ],
"countries": [ { "id": "3f2b8c1e-5a47-4d0e-9b6a-7c1d2e8f4a90", "name": "Aldoria" } ],
"people": [ { "id": "9a1c0d52-7be3-4f68-a2c4-1d5e8b3f6a07", "name": "Mara Voss", "title": "President",
              "title_type": "5c76242a-e7a0-4751-aa52-0d295a219590",
              "country": "3f2b8c1e-5a47-4d0e-9b6a-7c1d2e8f4a90" } ],
"newsreel": [
  { "subject": "3f2b8c1e-5a47-4d0e-9b6a-7c1d2e8f4a90", "type": "country_founded" },
  { "subject": "9a1c0d52-7be3-4f68-a2c4-1d5e8b3f6a07", "type": "person_joined" },
  { "subject": "47067f7a-fdac-4040-b652-873cebb0c53d", "type": "city_founded", "population": 100 }
]
```

- **Countries and people** carry forward: the next day's file starts with the same ones. The first country is the player's. Each person and each city belongs to a country by its id.
- **Ids:** every country, person and city has a GUID, and things refer to one another by it, never by name. A person's country is the country's GUID, and every news item stores the GUID of its **subject**, the primary thing it is about (the country founded, the person who joined, the city), in a `subject` field. The subject is only stored, for statistics: the newsreel never shows it, and its headlines look the names up when they are shown, so a name can change without breaking anything. Each kind of id is its own type in the code, so a country's id can't be used for a city. Ids are random GUIDs (version 4 UUIDs), made when the thing is founded.
- **The newsreel** belongs to its day alone: the next day starts with an empty one. Each item has a `type`, so statistics can count them by kind. Things like a country being founded are added to it, and it shows in the panel at the top right.

If the saved game has no country, a popup asks for the name of a new one when the game starts. Type the name and press Enter (or click **Found country**). The country is saved, and the day's newsreel reports it.

If there is a country but no people, a second popup asks for the name, the title and the **title type** of a person, and the person is added to the first country. Tab (or the arrow keys, or a click) moves between the fields, and Enter moves on to the next one.

**Title types.** A person's title is what they are called ("President", "Sea Mother"); the title type is what kind of title it is, such as Monarch, Nobility or Clergy. There are none to begin with. The title type field is a drop-down: **type a kind to make it** (a "New" tag shows that it isn't known yet), and it is added when the person is. After that it is in the list, so later people can pick it: click the field, or press Down, to see the list, and type to narrow it. Pick one with the mouse, or with Down and Enter. Esc closes an open list.

Title types are saved in each day's file (`title_types`) and carry forward, in the order they were added. A person refers to theirs by id (`title_type`); a kind gets a random GUID when it is first typed.

## Autosave

The game saves itself. Every 30 seconds, if anything in the current day has changed since it was last saved, the day's file is written, and it is written once more when the game closes (Esc or the window's close button). A day that hasn't changed is not rewritten. Founding a country is saved at once. If a save fails, a red notice under the top bar says why, and the game tries again at the next interval.
