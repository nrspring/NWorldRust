# NWorld – Game

The game. It loads the map from `Assets\map.nworld` in `C:\Users\nrspring\OneDrive\Secure\Projects\NWorld` (the map is made with the world editor in `WorldGeneration`, and lives outside the repository) and draws it with `nworld-view`. Pass another file as the first argument to use that instead.

```bash
cargo run --release -p nworld-game
```

| Control | |
|---|---|
| W A S D or arrows | Pan (hold Shift for faster) |
| Q / E | Turn |
| N, or click the compass in the header | Turn the map to look straight down with north at the top (it turns round the short way, from where you are and as far away as you are) |
| Wheel | Zoom |
| Right-drag | Look around and tilt |
| Left-drag | Grab the ground |
| Esc | Close the options, or quit |

Everything the game adds to the map, such as roads, cities and units, belongs to the game. `nworld-map` knows nothing about them. The map only answers questions about the ground.

## Header and footer

A **header** along the top of the window shows the player's country, once there is one, and the current day. At the right end of it is a **compass** whose red needle points to north as the camera sees it; **click it (or press N) to reset the view**: the camera turns, smoothly and the short way round, to look **straight down with north at the top of the screen**. It keeps its place and its distance, so only the view turns. North is the top edge of the map as the map file lays it out (z runs from north to south), which is not the way the game used to start: it looked towards the south edge, so the map began upside down to north. Turning or tilting by hand (Q/E, right-drag) stops the reset turn partway. Click the country to open the [countries menu](#countries-menu). A **footer** along the bottom holds the frame rate at the left, the keys in the middle (left out when the window is too narrow for them) and the overlays and options buttons at the right. While the mouse is over either, it doesn't move the camera, and a click there doesn't place a city. The newsreel and the save-failed notice sit below the header. A newsreel that is longer than about 45% of the window's height shows as many items as fit, with a note saying how many are above or below, and **the mouse wheel over it scrolls** through them an item at a time (it doesn't zoom the map).

## Countries menu

Clicking the country in the header opens a popup listing every country, with a filled mark beside the one you play.

- **Choose** a country by clicking its row (or the arrow keys and Enter). It becomes the player's: new people and cities belong to it, and the header shows it.
- **Edit** (the button on a row) asks for a new name. Everything that belongs to the country, and the newsreel, follows the rename, because they hold its id. Two countries can't share a name (capitals don't count).
- **The color square** beside a name is the country's color. Every country gets a random one when it is founded (picked to be unlike the others' colors), and clicking the square opens a color picker: a square for how rich and bright, a bar for the hue, and ten ready-made colors. Save color (or Enter) keeps it. The player's color also shows in the header. Colors are saved with the country as a hex code like `"#4f8cff"`.
- **Delete** (the cross on a row) removes a country that has **no cities**, after asking you to confirm. Its people go with it, the regions it owned become unowned, and the newsreel items about it and its people are dropped; if it was the country you play, the first country left is. A country with cities has no cross (give its cities to another country in the city popup first). Deleting the last country brings back the popup that asks you to found one.
- **Add a new country** asks for a name and adds it to the list, and to the newsreel. It doesn't become the player's until it is chosen.

Esc backs out of a rename or add to the list, and then closes the menu; so does a click outside it. Each change is saved as it is made. The country being played is kept in the day's file (`player`) and carries forward; a file without one plays the first country.

## Alerts

Under the newsreel, at the top right, is the alerts panel. It starts **collapsed**, as a bell icon; when there is something to see the bell turns amber and shows how many alerts there are. Click it to open a list, and click the header to collapse it again. **Click an alert** to open the [countries menu](#countries-menu) with the country it is about picked, ready to choose, edit or recolor.

Alerts are worked out from the game as it is, every frame, so they come and go as things change. They are not saved: there is nothing in the day's file for them, and they are never in the newsreel. The alerts so far:

- **A country has no cities**, one for each such country (so the first country has one until its first city is founded).

New alerts go in `Game/src/alerts.rs`: a variant of `Alert`, found in `Alert::current`, with its text.

## Ending the day

**Click the day** in the header (it lights up when the mouse is over it) to end it and start the next. A popup asks first ("Start the next day?", from day X to day Y). Only a **mouse click on Start day** goes ahead: no key does, so Enter can't end a day by accident. Cancel, a click outside, or Esc gives up. The popup says nothing more than that, and doesn't list the day's rules. Nothing advances the day by itself: it only moves when you confirm.

Going ahead saves the day that ended exactly as it was, writes the next day's file (it starts with the same countries, money, people, cities and region owners, and an empty newsreel), and then makes the new day the one the game is on in the main file, so a game that stops at any point can be opened again. Every day's file is kept. Days count through the calendar: 20 days to a month, 10 months to a year (1-1-20 is followed by 1-2-1, and 1-10-20 by 2-1-1).

## Day rules

When a day ends, the next day starts as a copy of it and then a series of **rules** runs on the new day, one after another in the order they are listed, so a later rule sees what the earlier ones did. The rules are in `Game/src/rules.rs` (`DAY_RULES`); each is a function with a sentence saying what it does (only for people reading the rules: the game doesn't show it). The day that ended is saved before they run and isn't changed by them.

The rules so far (the third, failures, is described under [Failures](#failures)):

1. **Income:** each country earns 1 coin for each person living in its cities, the total population of the cities it owns, which is added to its money.
2. **Population:** each city's population changes by a random amount. A city that is **not under siege grows by 1 to 3 percent**, and one that **is under siege shrinks by 5 to 10 percent** (each city gets its own random percent, anywhere in the range, not just whole numbers). The result is rounded to a whole number of people and kept between **100 and 100,000**: a city at 100 stays there under siege, and one at 100,000 stops growing. Because this comes after income, a country earns on the people its cities had at the end of the day, not the new ones. A city's size on the map follows its population, so cities that grow or shrink enough are redrawn.

Both rules report in the newsreel of the day they start: **"Aldoria earns 18,642 coins"** for each country that earned something (a country with no cities earns nothing and isn't mentioned), and **"Port Aldor grows by 20 to 1,020"**, **"Port Aldor loses 75 people and falls to 925"** or **"Port Aldor, under siege, loses 75 people and falls to 925"** for each city whose population changed (one that stayed the same, such as a besieged city already at 100, isn't mentioned). Each item is saved with the id of its country or city (`income` and `population_changed`, with the amounts), like the rest of the news.

## Money

Every country has money, in whole coins, which it has none of when it is founded (`STARTING_MONEY` in `country.rs` is 0). It can go below zero (debt, shown in red). It is part of the country in each day's file (`money`), so it carries forward from day to day and every day's file keeps what the country had then, for looking back on.

The **header** shows the money of the country you play, after the day, and the [countries menu](#countries-menu) shows each country's money beside its name. `DaySave::add_money` adds to or takes from a country's money; nothing earns or spends money yet, so it stays as it started.

## Paths

Paths are dirt paths laid over the map, a segment at a time. A **segment** is one of two kinds: from the **center of a region** to the **middle point of one of that region's edges** (the border it shares with a neighbor), or **straight across a region from the middle of one of its edges to the middle of another**, which doesn't go through the center. The center is the sample of the region nearest the average of its samples, so it always lies inside it, and the middle of an edge is the point of the border that is halfway along it. Both are worked out once, when the map loads, by `RegionMap` in `nworld-map` (`region_map.rs`), which stays free of anything that draws.

- **The tool.** The **Paths** button in the footer (left of Overlays) turns the path tool on. Markers then show on the map: **round** ones for centers and **square** ones for edges. At first they are the points of the region under the mouse. Click one to choose it as one end of a segment (it turns blue), and the markers that are left are the ones that can be the other end: after a center, the edges of its region; after an edge, the centers of the two regions it divides (each gives a different segment) **and their other edges** (an edge of either region gives a segment across that region, not through its center). A line shows the segment under the mouse, and clicking the other end builds it. Esc lets go of the end you chose, and Esc again turns the tool off. A click on nothing lets go too. The tool stays on to build more.
- **Where.** Paths go through dry land only (land and mountain regions, not the sea or deep water), and along edges between two such regions.
- **Paths can't cross rivers.** A segment that would have to cross a **river or a lake with no way round it**, or that has an end in the water, can't be built: while the mouse is on its other end the line turns **red** and the prompt says "A path can't cross a river or a lake.", and clicking it builds nothing and costs nothing. A lake that the path can go round is fine (it is drawn going round, below). The map doesn't tell a river from a lake, so both count.
- **Cost.** Each segment costs **10 coins** (either kind), charged to **the country you are playing**. Building one more than once is refused ("There is already a path there"), and a refused path costs nothing. A path is saved the moment it is built.
- **Not enough coin.** When the country you are playing has less than 10 coins, the Paths button is **dimmed and off**, with the reason in red beside it ("Not enough coin (needs 10, has 3)"), and the whole sentence when the mouse is over it. If the money runs out while the tool is on, it switches itself off the same way. A map with no regions has no paths either, and says so.
- **Not owned.** Paths belong to no country, so they don't change when the country you play does, and they carry forward from day to day. They are saved with the day as `paths`: the number of the region the segment is in (`region`), the neighbor whose shared edge it ends on (`neighbor`), and, for a segment across a region, the neighbor whose edge it starts from or ends on as well (`other`, the higher of the two numbers; left out for the kind that starts at the center). Paths saved before there were two kinds load as the first kind.
- **Look.** Each segment is drawn as a ribbon of packed dirt laid on the ground, about 7 m wide with soft edges and a little variation in its brown (`path_view.rs`, which is the game's own drawing, not part of the map's).
- **Never through water.** A segment is a straight line, and where that line would cross a lake, a river, or the sea (the shore of a concave coast), the path is drawn **going round it** instead: the shortest way over dry ground that keeps a few meters off the water, so the path's width stays on land (an A* search over a 6 m grid, pulled tight). Segments are only built when there is a way round (see above), so a new path is always one unbroken line. If there is no way round, which can only happen to a segment saved before this rule, it is drawn up to the water on each side and not over it. What it costs and what is saved don't change: it is still one segment, from the same two points, and the detour is only how it is drawn. Each segment's route is worked out once, when it first shows or when the map loads, not every frame.

## Roads

A **road** is a path made of packed stone. It is built, drawn and saved like a path (the same two kinds of segment, the same water rule, and the same links to the ends of bridges), but it costs **100 coins** a segment, charged to the country you are playing, and it looks different: **wider, in gray stone, with a darker curb along each side and blocks of slightly different shade one after another**, and it lies a little above a path so that it hides one under it.

- **The tool.** The **Roads** button in the footer (between Bridge and Paths) is the same tool as Paths, building roads. Every rule in [Paths](#paths) applies: clicking the ends, Esc, the red line and the message for a segment that would cross water, the markers for bridge ends, and being switched off, dimmed with the reason, when the country you are playing can't pay (here, with less than 100 coins). Only one of the Roads, Paths and Bridge tools is on at a time.
- **Paving a path.** Building a road on a segment that is already a path **replaces the path** (the road is paid for in full, and the path is removed from the data: it is no longer in the day's `paths` or `bridge_links`, so it isn't saved or carried forward). A path can't be built where there is a road ("There is already a road there."), and a road can't be built twice.
- **Saved.** Roads are saved with the day as `roads`, and the roads that start at a bridge's end as `road_links`, in the same form as `paths` and `bridge_links`. They belong to no country and carry forward. Days saved before roads load with none.

## Bridges

A **bridge** is a stone arch that carries a path across a river, which paths can't cross on their own. It costs **1,000 coins**, charged to the country you are playing, and belongs to no country. It is saved with the day as `bridges` (an id and the two ends) and carries forward.

- **The tool.** The **Bridge** button in the footer, left of Paths, turns the bridge tool on. Click dry ground by the water on one side of a river, then dry ground on the other side. A line from the first end to the mouse shows the bridge: **gold** if it can be built there, **red** if it can't, with the reason in the prompt. Esc lets go of the first end, and then turns the tool off. Only one of the Bridge, Roads and Paths tools is on at a time.
- **Where it can go.** Both ends have to be on dry ground (an end clicked on the bank is moved up to 10 m inland, a few meters from the water, so that a path can reach it); the ends can be at most **80 meters** apart; the line between them has to cross at least 3 m of **river or lake**, and **not the sea**; and it can't be a bridge that is already there.
- **Not enough coin.** With less than 1,000 coins the Bridge button is dimmed and off, with the reason in red beside the footer's buttons ("Bridges need 1,000 coins (has 250)") and the whole sentence over the button. If the money runs out while the tool is on, it switches itself off.
- **Look.** A stone arch (`bridge_view.rs`): the roadway rises from the ground at one end to a crown that clears the water by a couple of meters, and comes down at the other, with a low wall along each side and a slab under it, in gray stone with a little variation from block to block.
- **Paths connect to either end.** With the Paths tool on, the end of a bridge is a point you can click, shown as a gray block (it is offered in the region it stands in, with that region's other points). It joins to **the center of the region it is in** or **the middle of one of that region's edges**, as a segment of path that costs the usual **10 coins**. Such a segment can't cross water either. They are saved as `bridge_links` (the bridge, which end, the region and the neighbor). Bridge ends can be linked more than once, one for each place they join to.

## Failures

When a day ends, a rule lets **paths, roads and bridges fail**: each one that hasn't failed already has a random chance of failing on that day, **2% for a path, 0.1% for a road and 0.05% for a bridge** (`PATH_FAILURE`, `ROAD_FAILURE` and `BRIDGE_FAILURE` in `failure.rs`) (a path or road that starts at the end of a bridge counts as a path or a road). A segment that goes both ways, or across a region, is one thing and fails once.

- **What failing means.** The thing stays where it is, but it is **ruined**: a path or road is drawn dark and broken into pieces with gaps between them, and a bridge is darker and has **fallen in at the middle**. It is saved in the day as `failed` (the kind of thing and which one, or the bridge's id) and carries forward, so a ruin stays a ruin from day to day. Nothing uses paths, roads or bridges yet, so nothing else changes.
- **Alerts.** Each thing that has failed is an alert ("A path has failed", "A road has failed", "A bridge has failed"), after the alerts about countries with no cities, in the order they failed. **Click one to go to the region where it failed**: the camera flies to the middle of that region (for a bridge, the region it stands in). With more alerts than fit under the newsreel, the mouse wheel over the alerts scrolls through them.
- **Mending.** Build the same thing again and it is mended instead of there being a second one: the Paths and Roads tools take a click on a ruined segment (the prompt says "This path has failed. Click to build it again and mend it."), and the Bridge tool takes the same two places, for the usual cost (10, 100 or 1,000 coins). A bridge that has failed doesn't count as "already there" when planning a new one. A road built over a ruined path replaces it, and the ruin is removed with the path.

## Region names

Every region of the map, land or sea, can have a name. **Regions have no name to begin with**, and a region with no name has no label.

- **Showing them.** Turn on **Region names** in the [overlays menu](#overlays-menu). Each region that has a name is then labeled at its middle, in white text with a dark edge so that it reads over any ground. It is off when the game starts.
- **Naming a region.** With the overlay on, **click a region** (on the ground, not on a city, and when no tool is on) and a popup asks for its name: type it and press Enter (or click **Save name**). It says what kind of ground the region is (land, mountain, shallow water or deep water), and starts with the region's current name, so it works for renaming too. **Leave it blank to take the name away.** Esc closes it without changing anything. While the overlay is on, a note by the mouse says which region it is over, its name or "Unnamed region", and that clicking names it.
- **Names.** Up to 32 characters: letters, numbers, spaces, hyphens, apostrophes and full stops. Two regions may have the same name. Names are saved the moment you save them.
- **Saved.** The day's file has `region_names`: the region's number and its name, only for regions that have one. Names carry forward from day to day, and days saved before this load with none.

## Region ownership

Every region of the map can be owned by a country. They all start unowned. The day's file lists the owned ones (`region_owners`: the region's number and the owner's country id), which carries forward with the countries and cities.

- **A city makes its region its country's.** Founding a city claims the region it stands in for the city's country. Each city saves the number of its region.
- **A region with a city in it can't belong to anyone else.** You can't found a city in a region that another country owns (the placing prompt says "That region belongs to Brevon." and the click does nothing), and a region with a city in it can't be handed to another country or made unowned.
- **Owned regions are drawn** in their owner's color (a tint over the ground and a line in that color round them). The tint is see-through up close, so the ground shows, and gets more solid the farther away it is, from about 150 m to 3.5 km, so areas are easy to make out zoomed out. This is the **Country areas** overlay, on at the start and turned off in the [overlays menu](#overlays-menu). It works with or without the thin region lines of **Show regions**.

There is not yet a way to give regions to a country by hand; `DaySave::set_region_owner` does it, with those rules, for when there is.

## Overlays menu

The **Overlays** button in the footer, just left of the gear, opens a panel of layers drawn over the map, each with a switch. Click the button again, or press Esc, to close it. Only one of this and the options panel is open at a time.

- **Country areas**: the regions that countries own, in their colors (see [Region ownership](#region-ownership)). **On** when the game starts; greyed out, with a note, for a map that has no regions. Like the options, it isn't saved.
- **Region names**: the names the regions have been given, written on the map, and a way to change them (see [Region names](#region-names)). **Off** when the game starts.

## Options

The gear button at the right end of the footer opens the options panel above it. Click it again, or press Esc, to close it. While the mouse is over the panel, it doesn't move the camera.

- **Show regions**: draws thin lines between the map's regions, and leaves the ground as it is. It is **on** when the game starts. It is greyed out, with a note, for a map that has no regions. Turning it off lasts until the game closes; it isn't saved, so it is on again next time.

## Cities

A city has a name, belongs to a country, stands on the map, and has a population from 100 up to 100,000. Once the game has a country and a person but no city, it asks you to place the first one: move the mouse over the map, where a ring and a preview of the starting buildings show where it would go (green if it can, red if not, with the reason in the prompt), and click on dry land that is not up in the mountains. A drag still moves the map, so only a click places it. A popup then asks for the city's name (Esc gives up and goes back to placing it). The city starts with 100 people, belongs to the player's country, is reported in the newsreel, and the camera flies to it. Two cities can't have the same name.

**Hover over a city** to see a small panel with its name, its country and its population (with commas, like 12,345). A city counts as under the mouse when the mouse is over its buildings or just beside them, with a little extra room so a far-off city is not hard to point at.

**Click a city** to open its popup, which shows its name, its country (with the country's color) and its population. A click is a press and release that hardly moves, so dragging the map over a city doesn't open it. Esc, Enter, the Close button, or a click outside the popup closes it.

**Under siege:** every city has an *Under siege* switch in its popup, off when the city is founded. It is saved with the city (`under_siege`), carries forward from day to day, and each city has its own. Nothing in the game reacts to it yet: it is for the rules that will.

**Changing a city's country:** click the country row of the popup to open a list of the countries (the current one is marked), and click another to give the city to it. The city's region goes with it, so it is owned by the new country (see [Region ownership](#region-ownership)). If another city of the old country stands in the same region, the change is refused with a message saying which, since a region can only belong to one country. The change is saved as soon as it is made. Esc closes the list first, and then the popup.

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

- **Countries and people** carry forward: the next day's file starts with the same ones. The country being played (the first one founded, until another is chosen in the countries menu) is carried forward too. Each person and each city belongs to a country by its id.
- **Ids:** every country, person and city has a GUID, and things refer to one another by it, never by name. A person's country is the country's GUID, and every news item stores the GUID of its **subject**, the primary thing it is about (the country founded, the person who joined, the city), in a `subject` field. The subject is only stored, for statistics: the newsreel never shows it, and its headlines look the names up when they are shown, so a name can change without breaking anything. Each kind of id is its own type in the code, so a country's id can't be used for a city. Ids are random GUIDs (version 4 UUIDs), made when the thing is founded.
- **The newsreel** belongs to its day alone: the next day starts with an empty one. Each item has a `type`, so statistics can count them by kind. Things like a country being founded are added to it, and it shows in the panel at the top right.

If the saved game has no country, a popup asks for the name of a new one when the game starts. Type the name and press Enter (or click **Found country**). The country is saved, and the day's newsreel reports it.

If there is a country but no people, a second popup asks for the name, the title and the **title type** of a person, and the person is added to the player's country. Tab (or the arrow keys, or a click) moves between the fields, and Enter moves on to the next one.

**Title types.** A person's title is what they are called ("President", "Sea Mother"); the title type is what kind of title it is, such as Monarch, Nobility or Clergy. There are none to begin with. The title type field is a drop-down: **type a kind to make it** (a "New" tag shows that it isn't known yet), and it is added when the person is. After that it is in the list, so later people can pick it: click the field, or press Down, to see the list, and type to narrow it. Pick one with the mouse, or with Down and Enter. Esc closes an open list.

Title types are saved in each day's file (`title_types`) and carry forward, in the order they were added. A person refers to theirs by id (`title_type`); a kind gets a random GUID when it is first typed.

## Autosave

The game saves itself. Every 30 seconds, if anything in the current day has changed since it was last saved, the day's file is written, and it is written once more when the game closes (Esc or the window's close button). A day that hasn't changed is not rewritten. Founding a country is saved at once. If a save fails, a red notice at the top of the window says why, and the game tries again at the next interval.
