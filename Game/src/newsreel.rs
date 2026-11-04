//! The newsreel: what happened on a day. Each day has its own, which starts with the day beginning; nothing in it is
//! carried to the next day, but every day is kept, so the newsreels together are the history of the game.
//!
//! A news item has the id of its subject, the thing it is mainly about, and what happened to it. The
//! subject's id is stored with the item, for statistics, and is never shown: the headline looks the
//! names up in the day it is shown with, so a country's new name shows in its old news. What can't be looked up
//! later, because it was destroyed, renamed or paid for, is stored with the item itself.

use serde::{Deserialize, Serialize};

use crate::calendar::Date;
use crate::day::DaySave;
use crate::failure::Thing;
use crate::format::{capitalized, money, thousands, thousands_u64};
use crate::gatherer::GathererKind;
use crate::ids::{CityId, CountryId, EventId, GathererId, Guid, NoteId, PersonId};
use crate::rgb::Rgb;
use crate::text;
use crate::transfer::Asset;

/// What kind of thing was built, repaired, destroyed or failed.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Piece {
    Path,
    Road,
    Bridge,
}

impl Piece {
    pub fn noun(self) -> &'static str {
        match self {
            Self::Path => "path",
            Self::Road => "road",
            Self::Bridge => "bridge",
        }
    }
}

/// What a name was given to.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Named {
    Region,
    River,
    Lake,
    Place,
}

/// An amount of coins or a resource that something cost.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cost {
    pub asset: Asset,
    pub amount: u64,
}

impl Cost {
    /// `coins` coins, which is nothing if it is not more than that.
    pub fn coins(coins: i64) -> Self {
        Self { asset: Asset::Coins, amount: coins.max(0) as u64 }
    }
}

/// `amount` of `asset` as the newsreel says it: "1 coin", "1,000 coins", "500 stone".
fn amount_text(asset: Asset, amount: u64) -> String {
    match asset {
        Asset::Coins => format!("{} {}", thousands_u64(amount), if amount == 1 { "coin" } else { "coins" }),
        _ => format!("{} {}", thousands_u64(amount), asset.name()),
    }
}

/// What it cost, as a list: "10 coins", or "1,000 coins, 100 wood and 500 stone". Nothing for no cost.
pub fn cost_text(cost: &[Cost]) -> String {
    let parts: Vec<String> = cost.iter().map(|c| amount_text(c.asset, c.amount)).collect();
    match parts.as_slice() {
        [] => String::new(),
        [only] => only.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// " for 10 coins", or nothing at all if it cost nothing.
fn for_cost(cost: &[Cost]) -> String {
    let text = cost_text(cost);
    if text.is_empty() { text } else { format!(" for {text}") }
}

/// One thing that happened, saved with the id of its subject and a `type`, so statistics can count
/// and join the news by kind and by subject.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct News {
    /// The id of whatever the news is mainly about: the country founded, the person who joined, the
    /// city. Stored only; the newsreel doesn't show it.
    pub subject: Guid,
    #[serde(flatten)]
    pub event: Event,
}

/// What happened to the subject.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    /// The subject is a country, and it was founded.
    CountryFounded,
    /// The subject is a city of 100 people, which the country `country` settled in the region numbered `region` (if the
    /// map has regions) with people from its city `from`, for `cost`.
    CitySettled {
        population: u32,
        #[serde(default)]
        region: Option<u16>,
        country: CountryId,
        from: CityId,
        cost: Vec<Cost>,
    },
    /// The subject is a person, and they joined the country `country`. Their title and name are kept here too, since
    /// they go with their country if it is deleted. News saved before this has none of the three.
    PersonJoined {
        #[serde(default)]
        title: String,
        #[serde(default)]
        name: String,
        #[serde(default)]
        country: Option<CountryId>,
    },
    /// The subject is a kind of title, called `name`, and it was added when the first person of the kind joined.
    TitleTypeAdded { name: String },
    /// The subject is a city, and the country `country` founded it with this many people, in the region numbered
    /// `region` if the map has regions. A city can change countries later, so the founder is kept here; news saved
    /// before that has none.
    CityFounded {
        population: u32,
        #[serde(default)]
        region: Option<u16>,
        #[serde(default)]
        country: Option<CountryId>,
    },
    /// The subject is the country that owns the region numbered `region` now, or that did before if it has none now.
    /// The region went from the country `from`, if one had it, to `to`, if one has it.
    RegionOwnerChanged { region: u16, from: Option<CountryId>, to: Option<CountryId> },
    /// The subject is a city, and it went from the country `from` to the country `to`.
    CityChangedCountry { from: CountryId, to: CountryId },
    /// The subject is a city of `population` people, and it was put under siege or the siege was lifted.
    SiegeChanged { under_siege: bool, population: u32 },
    /// The subject is a country, and it had the name `from` and was renamed `to`.
    CountryRenamed { from: String, to: String },
    /// The subject is a country, and it changed color.
    CountryRecolored { from: Rgb, to: Rgb },
    /// The subject is a country, called `name`, and it was deleted. The news of it stays.
    CountryDeleted { name: String },
    /// The subject is the country the player now plays, which was `from` before, if there was one.
    PlayerChanged { from: Option<CountryId>, to: CountryId },
    /// The subject is the country that was sent `amount` of `asset` by the country `from`. (The sender has the news
    /// of [`Transfer`](Self::Transfer).)
    TransferReceived { from: CountryId, asset: Asset, amount: u64 },
    /// The subject is a path, a road or a bridge, which the country `country` paid `cost` for building in the regions
    /// `regions` (the two ends of a bridge). A road built where there was a path `paved` it.
    Built {
        piece: Piece,
        country: CountryId,
        regions: Vec<u16>,
        cost: Vec<Cost>,
        #[serde(default)]
        paved: bool,
    },
    /// The subject is a path, a road or a bridge that had failed, and the country `country` paid `cost` for building it
    /// again, which mended it.
    Repaired { piece: Piece, country: CountryId, regions: Vec<u16>, cost: Vec<Cost> },
    /// The subject is a path, a road or a bridge, which was destroyed by the country `country` being played, if there
    /// was one. A bridge takes `with_it` paths and roads that started at its ends with it.
    Destroyed {
        piece: Piece,
        country: Option<CountryId>,
        regions: Vec<u16>,
        #[serde(default)]
        with_it: u32,
    },
    /// The subject is a path, a road or a bridge, and it failed.
    Failed { piece: Piece, regions: Vec<u16> },
    /// The subject is nothing in particular, and a region, river, lake or place was named `to`, was called `from`
    /// before (nothing, if it had no name), and has `id` for a number. A name that is empty took the name away.
    NameChanged { what: Named, id: u32, from: String, to: String },
    /// The subject is nothing in particular, and the day with this date began.
    DayBegan { date: Date },
    /// The subject is a country, and it earned this many coins.
    Income { amount: i64 },
    /// The subject is a city, whose population went from `from` to `to` people, while it was or was not under
    /// siege.
    PopulationChanged { from: u32, to: u32, under_siege: bool },
    /// The subject is a country, and it sent this much of `asset` to the country `to`.
    Transfer { to: CountryId, asset: Asset, amount: u64 },
    /// The subject is a country, and its gatherers of `kind` that were operating provided this much of what they
    /// gather. News saved before there were kinds is of wood, and so is that of a gatherer, from when they were all
    /// woodcutters, under the names it had then.
    #[serde(alias = "wood_gathered")]
    Gathered { amount: u64, #[serde(default)] kind: GathererKind },
    /// The subject is a gatherer of `kind`, and the country `country` built it in the region numbered `region` for
    /// `cost`. News saved before these were kept has none of the three.
    #[serde(alias = "woodcutter_built")]
    GathererBuilt {
        #[serde(default)]
        kind: GathererKind,
        #[serde(default)]
        country: Option<CountryId>,
        #[serde(default)]
        region: Option<u16>,
        #[serde(default)]
        cost: Vec<Cost>,
    },
    /// The subject is a gatherer of `kind`, which belonged to the country `country`, and it was destroyed in the region
    /// numbered `region`. It is gone, so what the news says of it is kept here.
    #[serde(alias = "woodcutter_destroyed")]
    GathererDestroyed { country: CountryId, region: u16, #[serde(default)] kind: GathererKind },
    /// The subject is a gatherer of `kind` in the region numbered `region`, and it was transferred from the country
    /// `from` to the country `to`.
    #[serde(alias = "woodcutter_transferred")]
    GathererTransferred {
        from: CountryId,
        to: CountryId,
        #[serde(default)]
        kind: GathererKind,
        #[serde(default)]
        region: Option<u16>,
    },
    /// The subject is a gatherer of `kind`, which belongs to the country `country` and stands in the region numbered
    /// `region`, and it stopped operating because it lost its route to a city.
    #[serde(alias = "woodcutter_stopped")]
    GathererStopped {
        #[serde(default)]
        kind: GathererKind,
        #[serde(default)]
        country: Option<CountryId>,
        #[serde(default)]
        region: Option<u16>,
    },
    /// The subject is a gatherer of `kind`, which belongs to the country `country` and stands in the region numbered
    /// `region`, and it started operating, because it has a route to a city.
    #[serde(alias = "woodcutter_started")]
    GathererStarted {
        #[serde(default)]
        kind: GathererKind,
        #[serde(default)]
        country: Option<CountryId>,
        #[serde(default)]
        region: Option<u16>,
    },
    /// The subject is a militia of `members` people, which the country `country` raised in the city `city` for `cost`. The
    /// members came out of the city's population.
    MilitiaRaised {
        #[serde(default)]
        name: String,
        country: CountryId,
        city: CityId,
        members: u32,
        cost: Vec<Cost>,
    },
    /// The subject is a country, and it paid this many coins to keep its militia for a day.
    MilitiaPaid { amount: u64 },
    /// The subject is a militia called `name` of the country `country`, and `deserters` of its members deserted because the country
    /// couldn't pay them, leaving `remaining`. A militia with none left is gone.
    MilitiaDeserted {
        #[serde(default)]
        name: String,
        country: CountryId,
        deserters: u32,
        remaining: u32,
    },
    /// The subject is a militia of `members` people, and it went from the country `from` to the country `to`.
    MilitiaTransferred { from: CountryId, to: CountryId, members: u32 },
    /// The subject is a militia, which was called `from` and is called `to`.
    MilitiaRenamed { from: String, to: String },
    /// The subject is a militia called `name`, and it moved, which took `points` of its movement points, to somewhere in
    /// the map region numbered `region`, if the map has regions.
    MilitiaMoved {
        #[serde(default)]
        name: String,
        points: u32,
        #[serde(default)]
        region: Option<u16>,
    },
    /// The subject is a militia called `name`, which the militia called `absorbed` joined, making `members` of them.
    MilitiaMerged { name: String, absorbed: String, members: u32 },
    /// The subject is a militia called `name`, which split `members` of its people off into the new militia called
    /// `new_name`, leaving `remaining`.
    MilitiaSplit { name: String, new_name: String, members: u32, remaining: u32 },
    /// The subject is a militia called `name` of `members` people, which was disbanded. The members are gone: they don't
    /// go back to a city.
    MilitiaDisbanded { name: String, members: u32 },
    /// The subject is the note itself, which the player wrote, and this is what it says.
    Note { text: String },
}

/// The longest a note can be, in characters.
pub const MAX_NOTE_CHARS: usize = 140;

/// Whether `c` may be typed in a note: what a name allows, and the marks a sentence needs.
pub fn is_note_char(c: char) -> bool {
    text::is_text_char(c) || matches!(c, ',' | ';' | ':' | '!' | '?' | '"' | '(' | ')')
}

/// A note as typed, tidied up. An error says what is wrong with it.
pub fn clean_note(typed: &str) -> Result<String, String> {
    let text = typed.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.is_empty() {
        Err("Enter the note.".to_owned())
    } else if text.chars().count() > MAX_NOTE_CHARS {
        Err(format!("That's too long. Use {MAX_NOTE_CHARS} characters or fewer."))
    } else if !text.chars().all(is_note_char) {
        Err("Use only letters, numbers, spaces and ordinary punctuation.".to_owned())
    } else {
        Ok(text)
    }
}

impl Thing {
    /// What kind of thing this is, for the news.
    pub fn piece(self) -> Piece {
        match self {
            Self::Path(_) | Self::PathLink(_) => Piece::Path,
            Self::Road(_) | Self::RoadLink(_) => Piece::Road,
            Self::Bridge(_) => Piece::Bridge,
        }
    }
}

impl DaySave {
    /// What the news about `thing` is about and says: its subject (a bridge's id, or else nothing in particular), what
    /// kind it is, and the regions it is in, which for a bridge are those of its ends, if they were known.
    pub fn thing_news(&self, thing: Thing) -> (Guid, Piece, Vec<u16>) {
        match thing {
            Thing::Path(segment) | Thing::Road(segment) => (EventId::new().into(), thing.piece(), vec![segment.region]),
            Thing::PathLink(link) | Thing::RoadLink(link) => (EventId::new().into(), thing.piece(), vec![link.region]),
            Thing::Bridge(id) => (id.into(), Piece::Bridge, self.bridge(id).map_or_else(Vec::new, |b| b.regions.iter().flatten().copied().collect())),
        }
    }

    /// Adds a note saying `typed` to today's newsreel. An error says why it can't be done.
    pub fn add_note(&mut self, typed: &str) -> Result<(), String> {
        let text = clean_note(typed)?;
        self.newsreel.push(News::about(NoteId::new(), Event::Note { text }));
        Ok(())
    }
}

/// " in region 3" for a path or a road, which is in one region, and " between region 3 and region 4" for a bridge, which
/// joins two; nothing if the regions aren't known.
fn place_text(day: &DaySave, piece: Piece, regions: &[u16]) -> String {
    match (piece, regions) {
        (Piece::Bridge, [first, second, ..]) if first != second => format!(" between {} and {}", day.region_label(*first), day.region_label(*second)),
        (_, [first, ..]) => format!(" in {}", day.region_label(*first)),
        (_, []) => String::new(),
    }
}

impl News {
    /// News of `event` happening to `subject`, whose id of any kind will do.
    pub fn about(subject: impl Into<Guid>, event: Event) -> Self {
        Self { subject: subject.into(), event }
    }

    /// The news of a happening that is about no one thing with an id of its own.
    pub fn happening(event: Event) -> Self {
        Self::about(EventId::new(), event)
    }

    /// The country and the region of the gatherer this is about: what the news kept, or else what the gatherer says if
    /// it is still there.
    fn gatherer_whereabouts(&self, day: &DaySave, country: Option<CountryId>, region: Option<u16>) -> Option<(CountryId, u16)> {
        let live = day.gatherer(GathererId::from(self.subject));
        Some((country.or(live.map(|g| g.country))?, region.or(live.map(|g| g.region))?))
    }

    /// The line the newsreel shows for it, with the names looked up in `day`. It says nothing of ids.
    pub fn headline(&self, day: &DaySave) -> String {
        match self.event {
            Event::CountryFounded => format!("The nation of {} is founded", day.country_name(CountryId::from(self.subject))),
            Event::PersonJoined { ref title, ref name, country } => match day.person(PersonId::from(self.subject)) {
                Some(person) => format!("{} {} joins {}", person.title, person.name, day.country_name(person.country)),
                // Gone with their country: what was kept says who they were.
                None if !name.is_empty() => match country {
                    Some(country) => format!("{title} {name} joins {}", day.country_name(country)),
                    None => format!("{title} {name} joins a country"),
                },
                None => "Someone joins a country".to_owned(),
            },
            Event::TitleTypeAdded { ref name } => format!("A new kind of title, {name}, is added"),
            Event::CityFounded { population, region, country } => {
                let place = region.map_or_else(String::new, |region| format!(" in {}", day.region_label(region)));
                let city = day.city(CityId::from(self.subject));
                match (city, country.or(city.map(|city| city.country))) {
                    (Some(city), Some(country)) => format!("{} founds {}{place}, a city of {population} people", day.country_name(country), city.name),
                    _ => format!("A city{place} of {population} people is founded"),
                }
            }
            Event::CitySettled { population, region, country, from, ref cost } => {
                let place = region.map_or_else(String::new, |region| format!(" in {}", day.region_label(region)));
                let name = day.city(CityId::from(self.subject)).map_or("a new city", |city| city.name.as_str());
                let origin = day.city(from).map_or("another city", |city| city.name.as_str());
                format!("{} settles {name}{place}, a city of {population} people from {origin}{}", day.country_name(country), for_cost(cost))
            }
            Event::RegionOwnerChanged { region, from, to } => {
                let label = capitalized(&day.region_label(region));
                match (from, to) {
                    (None, Some(to)) => format!("{} claims {}", day.country_name(to), day.region_label(region)),
                    (Some(from), None) => format!("{} gives up {}", day.country_name(from), day.region_label(region)),
                    (Some(from), Some(to)) => format!("{label} passes from {} to {}", day.country_name(from), day.country_name(to)),
                    (None, None) => format!("{label} has no country"),
                }
            }
            Event::CityChangedCountry { from, to } => {
                let name = day.city(CityId::from(self.subject)).map_or("A city", |city| city.name.as_str());
                format!("{name} passes from {} to {}", day.country_name(from), day.country_name(to))
            }
            Event::SiegeChanged { under_siege, population } => {
                let name = day.city(CityId::from(self.subject)).map_or("A city", |city| city.name.as_str());
                if under_siege {
                    format!("{name}, a city of {}, is put under siege", thousands(population))
                } else {
                    format!("The siege of {name}, a city of {}, is lifted", thousands(population))
                }
            }
            Event::CountryRenamed { ref from, ref to } => format!("{from} is renamed {to}"),
            Event::CountryRecolored { from, to } => format!("{} changes its color from {} to {}", day.country_name(CountryId::from(self.subject)), from.to_hex(), to.to_hex()),
            Event::CountryDeleted { ref name } => format!("The nation of {name} is deleted"),
            Event::PlayerChanged { from, to } => match from {
                Some(from) => format!("The player switches from {} to {}", day.country_name(from), day.country_name(to)),
                None => format!("The player now plays {}", day.country_name(to)),
            },
            Event::TransferReceived { from, asset, amount } => format!("{} receives {} from {}", day.country_name(CountryId::from(self.subject)), amount_text(asset, amount), day.country_name(from)),
            Event::Built { piece, country, ref regions, ref cost, paved } => {
                let (country, place, cost) = (day.country_name(country), place_text(day, piece, regions), for_cost(cost));
                if paved {
                    format!("{country} paves a path{place} into a road{cost}")
                } else {
                    format!("{country} builds a {}{place}{cost}", piece.noun())
                }
            }
            Event::Repaired { piece, country, ref regions, ref cost } => {
                format!("{} repairs a {}{}{}", day.country_name(country), piece.noun(), place_text(day, piece, regions), for_cost(cost))
            }
            Event::Destroyed { piece, country, ref regions, with_it } => {
                let place = place_text(day, piece, regions);
                let mut line = match country {
                    Some(country) => format!("{} destroys a {}{place}", day.country_name(country), piece.noun()),
                    None => format!("A {}{place} is destroyed", piece.noun()),
                };
                if with_it > 0 {
                    line.push_str(&format!(", taking {with_it} {} that started at its ends with it", if with_it == 1 { "segment of path or road" } else { "segments of path or road" }));
                }
                line
            }
            Event::Failed { piece, ref regions } => format!("A {}{} fails", piece.noun(), place_text(day, piece, regions)),
            Event::NameChanged { what, id, ref from, ref to } => {
                let subject = match what {
                    Named::Region => format!("Region {}", id & 0x3FFF),
                    Named::River => "A river".to_owned(),
                    Named::Lake => "A lake".to_owned(),
                    Named::Place => "A place".to_owned(),
                };
                match (from.is_empty(), to.is_empty()) {
                    (true, _) => format!("{subject} is named {to}"),
                    (false, true) if what == Named::Place => format!("The place called {from} is removed"),
                    (false, true) => format!("{subject} called {from} loses its name"),
                    (false, false) => format!("{subject} called {from} is renamed {to}"),
                }
            }
            Event::DayBegan { date } => format!("Day {date} begins"),
            Event::Income { amount } => format!("{} earns {} coins", day.country_name(CountryId::from(self.subject)), money(amount)),
            Event::PopulationChanged { from, to, under_siege } => {
                let name = day.city(CityId::from(self.subject)).map_or("A city", |city| city.name.as_str());
                let change = thousands(from.abs_diff(to));
                let now = thousands(to);
                match (to > from, under_siege) {
                    (true, _) => format!("{name} grows by {change} to {now}"),
                    (false, true) => format!("{name}, under siege, loses {change} people and falls to {now}"),
                    (false, false) => format!("{name} loses {change} people and falls to {now}"),
                }
            }
            Event::Transfer { to, asset, amount } => {
                format!("{} sends {} to {}", day.country_name(CountryId::from(self.subject)), amount_text(asset, amount), day.country_name(to))
            }
            Event::Gathered { amount, kind } => format!("{} gathers {} {} from its {}", day.country_name(CountryId::from(self.subject)), thousands_u64(amount), kind.product(), kind.plural()),
            Event::GathererBuilt { kind, country, region, ref cost } => {
                // What was kept when it was built, and for news from before that, the gatherer itself if it is still there.
                let live = day.gatherer(GathererId::from(self.subject));
                match (country.or(live.map(|g| g.country)), region.or(live.map(|g| g.region))) {
                    (Some(country), Some(region)) => format!("{} builds {} in {}{}", day.country_name(country), kind.a_noun(), day.region_label(region), for_cost(cost)),
                    _ => format!("{} is built{}", kind.a_noun_capital(), for_cost(cost)),
                }
            }
            Event::GathererDestroyed { country, region, kind } => format!("{}'s {} in {} is destroyed", day.country_name(country), kind.noun(), day.region_label(region)),
            Event::GathererTransferred { from, to, kind, region } => match region.or(day.gatherer(GathererId::from(self.subject)).map(|g| g.region)) {
                Some(region) => format!("The {} in {} is transferred from {} to {}", kind.noun(), day.region_label(region), day.country_name(from), day.country_name(to)),
                None => format!("{} is transferred from {} to {}", kind.a_noun_capital(), day.country_name(from), day.country_name(to)),
            },
            Event::GathererStopped { kind, country, region } => match self.gatherer_whereabouts(day, country, region) {
                Some((country, region)) => format!("{}'s {} in {} stops operating, with no route to a city", day.country_name(country), kind.noun(), day.region_label(region)),
                None => format!("{} stops operating", kind.a_noun_capital()),
            },
            Event::GathererStarted { kind, country, region } => match self.gatherer_whereabouts(day, country, region) {
                Some((country, region)) => format!("{}'s {} in {} starts operating", day.country_name(country), kind.noun(), day.region_label(region)),
                None => format!("{} starts operating", kind.a_noun_capital()),
            },
            Event::MilitiaRaised { ref name, country, city, members, ref cost } => {
                let place = day.city(city).map_or_else(|| "a city".to_owned(), |city| city.name.clone());
                let name = if name.is_empty() { "a militia" } else { name };
                format!("{} raises {name} of {} in {place}{}", day.country_name(country), thousands(members), for_cost(cost))
            }
            Event::MilitiaPaid { amount } => format!("{} pays {} to keep its militia", day.country_name(CountryId::from(self.subject)), amount_text(Asset::Coins, amount)),
            Event::MilitiaDeserted { ref name, country, deserters, remaining } => {
                let deserters = thousands(deserters);
                let name = if name.is_empty() { format!("{}'s militia", day.country_name(country)) } else { name.clone() };
                if remaining == 0 {
                    format!("All {deserters} members of {name} desert for want of pay, and it is gone")
                } else {
                    format!("{deserters} members of {name} desert for want of pay, leaving {}", thousands(remaining))
                }
            }
            Event::MilitiaTransferred { from, to, members } => {
                let name = day.militia(crate::ids::MilitiaId::from(self.subject)).map_or_else(|| format!("A militia of {}", thousands(members)), |m| m.title().to_owned());
                format!("{name} passes from {} to {}", day.country_name(from), day.country_name(to))
            }
            Event::MilitiaRenamed { ref from, ref to } => format!("{from} is renamed {to}"),
            Event::MilitiaMoved { ref name, points, region } => {
                let name = if name.is_empty() { "A militia" } else { name };
                let spent = format!("{} movement {}", thousands(points), if points == 1 { "point" } else { "points" });
                match region {
                    Some(region) => format!("{name} marches to {}, spending {spent}", day.region_label(region)),
                    None => format!("{name} marches, spending {spent}"),
                }
            }
            Event::MilitiaMerged { ref name, ref absorbed, members } => format!("{absorbed} joins {name}, making {} members", thousands(members)),
            Event::MilitiaSplit { ref name, ref new_name, members, remaining } => {
                format!("{name} splits off {} members as {new_name}, leaving {}", thousands(members), thousands(remaining))
            }
            Event::MilitiaDisbanded { ref name, members } => format!("{name} is disbanded, and its {} disappear{}", if members == 1 { "1 member".to_owned() } else { format!("{} members", thousands(members)) }, if members == 1 { "s" } else { "" }),
            Event::Note { ref text } => text.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::Date;

    fn day() -> DaySave {
        let mut day = DaySave::new(Date::FIRST, "Assets/map.nworld");
        day.found_country("Aldoria").unwrap();
        day.found_person("Mara Voss", "President", "Head of State").unwrap();
        day.found_city("Port Aldor", 0.0, 0.0, None).unwrap();
        day
    }

    #[test]
    fn each_kind_of_news_makes_a_headline_from_the_names() {
        let day = day();
        let headlines: Vec<String> = day.newsreel.iter().map(|n| n.headline(&day)).collect();
        assert_eq!(
            headlines,
            ["The nation of Aldoria is founded", "A new kind of title, Head of State, is added", "President Mara Voss joins Aldoria", "Aldoria founds Port Aldor, a city of 100 people"]
        );
    }

    #[test]
    fn income_and_population_news_read_naturally() {
        let day = day();
        let (aldoria, port) = (day.countries[0].id, day.cities[0].id);
        let income = |amount| News::about(aldoria, Event::Income { amount }).headline(&day);
        assert_eq!(income(1_200), "Aldoria earns 1,200 coins");
        let changed = |from, to, under_siege| News::about(port, Event::PopulationChanged { from, to, under_siege }).headline(&day);
        assert_eq!(changed(1_000, 1_020, false), "Port Aldor grows by 20 to 1,020");
        assert_eq!(changed(1_000, 925, true), "Port Aldor, under siege, loses 75 people and falls to 925");
        assert_eq!(changed(10_000, 9_500, false), "Port Aldor loses 500 people and falls to 9,500");
    }

    #[test]
    fn income_and_population_news_is_saved_with_what_changed() {
        let income = News::about(CountryId::from_u128(1), Event::Income { amount: 250 });
        let json = serde_json::to_string(&income).unwrap();
        assert!(json.contains(r#""type":"income""#) && json.contains(r#""amount":250"#), "{json}");
        assert_eq!(serde_json::from_str::<News>(&json).unwrap(), income);
        let population = News::about(CityId::from_u128(2), Event::PopulationChanged { from: 100, to: 102, under_siege: false });
        let json = serde_json::to_string(&population).unwrap();
        assert!(json.contains(r#""type":"population_changed""#) && json.contains(r#""from":100"#) && json.contains(r#""to":102"#) && json.contains(r#""under_siege":false"#), "{json}");
        assert_eq!(serde_json::from_str::<News>(&json).unwrap(), population);
    }

    #[test]
    fn news_about_something_that_is_not_there_still_reads_sensibly() {
        let day = DaySave::new(Date::FIRST, "Assets/map.nworld");
        assert_eq!(News::about(CountryId::new(), Event::CountryFounded).headline(&day), "The nation of an unknown country is founded");
        assert_eq!(News::about(PersonId::new(), Event::PersonJoined { title: String::new(), name: String::new(), country: None }).headline(&day), "Someone joins a country");
        assert_eq!(News::about(CityId::new(), Event::CityFounded { population: 100, region: None, country: None }).headline(&day), "A city of 100 people is founded");
        assert_eq!(News::about(CountryId::new(), Event::Income { amount: 5 }).headline(&day), "an unknown country earns 5 coins");
        assert_eq!(News::about(CityId::new(), Event::PopulationChanged { from: 100, to: 110, under_siege: false }).headline(&day), "A city grows by 10 to 110");
    }

    #[test]
    fn a_note_is_tidied_and_added_to_the_newsreel_as_it_was_written() {
        let mut day = day();
        let before = day.newsreel.len();
        day.add_note("  The  harvest, at last!  ").unwrap();
        assert_eq!(day.newsreel.len(), before + 1);
        let note = day.newsreel.last().unwrap();
        assert_eq!(note.headline(&day), "The harvest, at last!");
        assert!(matches!(note.event, Event::Note { .. }));
    }

    #[test]
    fn a_bad_note_is_refused_and_changes_nothing() {
        let mut day = day();
        let before = day.clone();
        assert!(day.add_note("   ").unwrap_err().contains("note"));
        assert!(day.add_note("no <tags>").is_err());
        assert!(day.add_note(&"a".repeat(MAX_NOTE_CHARS + 1)).unwrap_err().contains("too long"));
        assert_eq!(day, before);
        assert!(day.add_note(&"a".repeat(MAX_NOTE_CHARS)).is_ok());
    }

    #[test]
    fn a_note_is_saved_with_its_text_and_its_own_id_and_stays_with_its_day() {
        let mut today = day();
        today.add_note("Peace talks begin").unwrap();
        let json = serde_json::to_string(today.newsreel.last().unwrap()).unwrap();
        assert!(json.contains(r#""type":"note""#) && json.contains(r#""text":"Peace talks begin""#), "{json}");
        assert_eq!(serde_json::from_str::<News>(&json).unwrap(), *today.newsreel.last().unwrap());
        assert!(today.following(crate::calendar::Date { year: 1, month: 1, day: 2 }).newsreel.is_empty(), "like all news, it is for its day only");
    }

    /// The headlines of the news from the `from`th item on.
    fn since(day: &DaySave, from: usize) -> Vec<String> {
        day.newsreel[from..].iter().map(|n| n.headline(day)).collect()
    }

    fn two_countries() -> DaySave {
        let mut day = DaySave::new(Date::FIRST, "Assets/map.nworld");
        day.found_country("Aldoria").unwrap();
        day.found_country("Brevon").unwrap();
        day
    }

    #[test]
    fn building_paving_repairing_failing_and_destroying_are_each_in_the_newsreel() {
        use crate::bridge::{BridgeLink, End, Spot};
        use crate::path::{Kind, PathSegment};
        let mut day = two_countries();
        let aldoria = day.countries[0].id;
        day.add_money(aldoria, 5_000).unwrap();
        day.stock_for_bridges();
        let segment = PathSegment::to_edge(5, 6);

        let from = day.newsreel.len();
        day.build_segment(Kind::Path, segment).unwrap();
        day.build_segment(Kind::Road, segment).unwrap();
        day.fail(Thing::Road(segment));
        day.fail(Thing::Road(segment));
        day.build_segment(Kind::Road, segment).unwrap();
        assert_eq!(
            since(&day, from),
            [
                "Aldoria builds a path in region 5 for 10 coins",
                "Aldoria paves a path in region 5 into a road for 100 coins",
                "A road in region 5 fails",
                "Aldoria repairs a road in region 5 for 100 coins",
            ],
            "a thing that has failed already doesn't fail again"
        );
        // Saved with what it cost, and with the country that paid as the subject.
        let built = &day.newsreel[from];
        assert_eq!(built.subject, aldoria.into());
        let json = serde_json::to_string(built).unwrap();
        assert!(json.contains(r#""type":"built""#) && json.contains(r#""piece":"path""#) && json.contains(r#""asset":"coins""#) && json.contains(r#""amount":10"#), "{json}");
        assert_eq!(serde_json::from_str::<News>(&json).unwrap(), *built);
        // A segment that can't be built is not news.
        let count = day.newsreel.len();
        assert!(day.build_segment(Kind::Path, segment).is_err());
        assert_eq!(day.newsreel.len(), count);

        // A bridge, which is paid for in wood and stone too, with a path from one end.
        let from = day.newsreel.len();
        let (first, second) = (Spot { x: 0.0, z: 0.0 }, Spot { x: 40.0, z: 0.0 });
        day.build_bridge_in(first, second, [Some(5), Some(6)]).unwrap();
        let bridge = day.bridges[0].id;
        day.build_link(BridgeLink { bridge, end: End::First, region: 5, to: None }).unwrap();
        day.fail(Thing::Bridge(bridge));
        day.build_bridge_in(first, second, [Some(5), Some(6)]).unwrap();
        assert_eq!(
            since(&day, from),
            [
                "Aldoria builds a bridge between region 5 and region 6 for 1,000 coins, 100 wood and 500 stone",
                "Aldoria builds a path in region 5 for 10 coins",
                "A bridge between region 5 and region 6 fails",
                "Aldoria repairs a bridge between region 5 and region 6 for 1,000 coins, 100 wood and 500 stone",
            ]
        );
        assert_eq!(day.bridges.len(), 1, "mended, not built again");
        assert_eq!(day.newsreel[from].subject, bridge.into(), "the bridge is the subject of its own news");
        // One built without its regions known says nothing of where.
        day.build_bridge(Spot { x: 500.0, z: 0.0 }, Spot { x: 540.0, z: 0.0 }).unwrap();
        assert_eq!(day.newsreel.last().unwrap().headline(&day), "Aldoria builds a bridge for 1,000 coins, 100 wood and 500 stone");

        // Destroying says by whom, and what a bridge took with it.
        let from = day.newsreel.len();
        day.destroy(Thing::Road(segment)).unwrap();
        day.destroy(Thing::Bridge(bridge)).unwrap();
        assert_eq!(
            since(&day, from),
            ["Aldoria destroys a road in region 5", "Aldoria destroys a bridge between region 5 and region 6, taking 1 segment of path or road that started at its ends with it"]
        );
        assert_eq!(day.newsreel[from + 1].subject, bridge.into());
        let count = day.newsreel.len();
        assert!(day.destroy(Thing::Road(segment)).is_err());
        assert_eq!(day.newsreel.len(), count, "one that isn't there is not news");
        let no_country = Event::Destroyed { piece: Piece::Path, country: None, regions: vec![9], with_it: 0 };
        assert_eq!(News::happening(no_country).headline(&day), "A path in region 9 is destroyed");
    }

    #[test]
    fn the_things_that_fail_when_a_day_ends_are_in_the_new_days_newsreel() {
        use crate::path::{Kind, PathSegment};
        let mut day = two_countries();
        day.add_money(day.countries[0].id, 100).unwrap();
        day.build_segment(Kind::Path, PathSegment::to_edge(5, 6)).unwrap();
        // Every path fails, whatever the chance.
        let mut next = day.following(Date { year: 1, month: 1, day: 2 });
        for thing in next.things() {
            next.fail(thing);
        }
        assert_eq!(since(&next, 0), ["A path in region 5 fails"]);
    }

    #[test]
    fn a_day_begins_with_the_news_that_it_began() {
        let day = two_countries().next_day();
        assert_eq!(day.newsreel[0].headline(&day), format!("Day {} begins", day.date));
        assert_eq!(day.newsreel[0].event, Event::DayBegan { date: day.date });
    }

    #[test]
    fn who_has_a_region_is_in_the_newsreel_whichever_way_it_changes_hands() {
        let mut day = two_countries();
        let (aldoria, brevon) = (day.countries[0].id, day.countries[1].id);
        let from = day.newsreel.len();
        day.claim_region(0x4005).unwrap();
        day.claim_region(0x4005).unwrap();
        day.assign_region(0x4005, "No country").unwrap();
        day.assign_region(0x4005, "Brevon").unwrap();
        day.found_city("Port Aldor", 0.0, 0.0, Some(0x4006)).unwrap();
        let city = day.cities[0].id;
        day.set_city_country(city, brevon).unwrap();
        day.set_city_country(city, brevon).unwrap();
        assert_eq!(
            since(&day, from),
            [
                "Aldoria claims region 5",
                "Aldoria gives up region 5",
                "Brevon claims region 5",
                "Aldoria founds Port Aldor in region 6, a city of 100 people",
                "Aldoria claims region 6",
                "Port Aldor passes from Aldoria to Brevon",
                "Region 6 passes from Aldoria to Brevon",
            ],
            "doing again what is done is not news"
        );
        assert_eq!(day.newsreel[from].subject, aldoria.into());
        assert_eq!(day.newsreel[from + 2].subject, brevon.into());
        assert_eq!(day.newsreel[from + 5].subject, city.into());
        let json = serde_json::to_string(&day.newsreel[from]).unwrap();
        assert!(json.contains(r#""type":"region_owner_changed""#) && json.contains(r#""region":16389"#) && json.contains(r#""to":""#), "{json}");
        assert_eq!(serde_json::from_str::<News>(&json).unwrap(), day.newsreel[from]);
    }

    #[test]
    fn naming_a_region_a_river_a_lake_or_a_place_is_in_the_newsreel() {
        use macroquad::prelude::vec2;
        let mut day = two_countries();
        let from = day.newsreel.len();
        day.set_region_name(0x4005, "The Marches").unwrap();
        day.set_region_name(0x4005, "The Marches").unwrap();
        day.set_region_name(0x4005, "The Borderlands").unwrap();
        day.set_region_name(0x4005, "").unwrap();
        day.set_river_name(7, "Silverrun").unwrap();
        day.set_river_name(7, "Goldrun").unwrap();
        day.set_lake_name(9, "Lake Calm").unwrap();
        day.set_lake_name(9, "").unwrap();
        let id = day.add_place(vec2(1.0, 2.0), "Bay of Storms").unwrap();
        day.set_place_name(id, "Storm Bay").unwrap();
        day.set_place_name(id, "").unwrap();
        assert_eq!(
            since(&day, from),
            [
                "Region 5 is named The Marches",
                "Region 5 called The Marches is renamed The Borderlands",
                "Region 5 called The Borderlands loses its name",
                "A river is named Silverrun",
                "A river called Silverrun is renamed Goldrun",
                "A lake is named Lake Calm",
                "A lake called Lake Calm loses its name",
                "A place is named Bay of Storms",
                "A place called Bay of Storms is renamed Storm Bay",
                "The place called Storm Bay is removed",
            ],
            "the same name again is not news"
        );
        // A name that is refused is not news.
        let count = day.newsreel.len();
        assert!(day.set_river_name(7, "no <tags>").is_err());
        assert_eq!(day.newsreel.len(), count);
        let json = serde_json::to_string(&day.newsreel[from]).unwrap();
        assert!(json.contains(r#""type":"name_changed""#) && json.contains(r#""what":"region""#) && json.contains(r#""to":"The Marches""#), "{json}");
        assert_eq!(serde_json::from_str::<News>(&json).unwrap(), day.newsreel[from]);
    }

    #[test]
    fn a_siege_beginning_or_ending_is_in_the_newsreel() {
        let mut day = day();
        let city = day.cities[0].id;
        let from = day.newsreel.len();
        day.set_city_under_siege(city, false).unwrap();
        day.set_city_under_siege(city, true).unwrap();
        day.set_city_under_siege(city, true).unwrap();
        day.set_city_under_siege(city, false).unwrap();
        assert_eq!(since(&day, from), ["Port Aldor, a city of 100, is put under siege", "The siege of Port Aldor, a city of 100, is lifted"]);
        assert_eq!(day.newsreel[from].subject, city.into());
    }

    #[test]
    fn a_country_being_renamed_recolored_switched_to_or_deleted_is_in_the_newsreel() {
        let mut day = two_countries();
        let (aldoria, brevon) = (day.countries[0].id, day.countries[1].id);
        let from = day.newsreel.len();
        day.rename_country(aldoria, "Aldoria").unwrap();
        day.rename_country(aldoria, "New Aldoria").unwrap();
        let before = day.country(aldoria).unwrap().color;
        day.recolor_country(aldoria, before).unwrap();
        day.recolor_country(aldoria, Rgb { r: 1, g: 2, b: 3 }).unwrap();
        day.select_country(aldoria).unwrap();
        day.select_country(brevon).unwrap();
        day.delete_country(aldoria).unwrap();
        assert_eq!(
            since(&day, from),
            [
                "Aldoria is renamed New Aldoria".to_owned(),
                format!("New Aldoria changes its color from {} to #010203", before.to_hex()),
                "The player switches from New Aldoria to Brevon".to_owned(),
                "The nation of New Aldoria is deleted".to_owned(),
            ]
        );
        // Its old news still names it, now that it is gone.
        assert_eq!(day.newsreel[from + 1].headline(&day), format!("New Aldoria changes its color from {} to #010203", before.to_hex()));
        assert_eq!(day.newsreel[0].headline(&day), "The nation of New Aldoria is founded");
        let mut one = DaySave::new(Date::FIRST, "x");
        one.found_country("Solo").unwrap();
        one.found_country("Duo").unwrap();
        let solo = one.countries[0].id;
        assert_eq!(News::about(solo, Event::PlayerChanged { from: None, to: solo }).headline(&one), "The player now plays Solo");
    }

    #[test]
    fn the_person_of_a_deleted_country_is_still_named_in_their_news() {
        let mut day = two_countries();
        let brevon = day.countries[1].id;
        day.select_country(brevon).unwrap();
        day.found_person("Tomas Rue", "King", "Monarch").unwrap();
        day.delete_country(brevon).unwrap();
        let joined = day.newsreel.iter().find(|n| matches!(n.event, Event::PersonJoined { .. })).unwrap();
        assert_eq!(joined.headline(&day), "King Tomas Rue joins Brevon");
    }

    #[test]
    fn news_saved_before_these_details_were_kept_still_loads_and_reads() {
        let day = day();
        let subject = day.people[0].id;
        let old: News = serde_json::from_str(&format!(r#"{{"subject":"{subject}","type":"person_joined"}}"#)).unwrap();
        assert_eq!(old.headline(&day), "President Mara Voss joins Aldoria", "from the person, who is there");
        let old: News = serde_json::from_str(&format!(r#"{{"subject":"{}","type":"city_founded","population":100}}"#, day.cities[0].id)).unwrap();
        assert_eq!(old.headline(&day), "Aldoria founds Port Aldor, a city of 100 people");
        let gone = GathererId::new();
        let old: News = serde_json::from_str(&format!(r#"{{"subject":"{gone}","type":"gatherer_stopped","kind":"woodcutter"}}"#)).unwrap();
        assert_eq!(old.headline(&day), "A woodcutter stops operating");
        // And what is kept now outlives the gatherer.
        let aldoria = day.countries[0].id;
        let kept = News::about(gone, Event::GathererStopped { kind: GathererKind::Woodcutter, country: Some(aldoria), region: Some(3) });
        assert_eq!(kept.headline(&day), "Aldoria's woodcutter in region 3 stops operating, with no route to a city");
        let kept = News::about(gone, Event::GathererStarted { kind: GathererKind::Woodcutter, country: Some(aldoria), region: Some(3) });
        assert_eq!(kept.headline(&day), "Aldoria's woodcutter in region 3 starts operating");
    }

    #[test]
    fn every_news_item_has_the_id_of_its_subject() {
        let day = day();
        let subjects: Vec<Guid> = day.newsreel.iter().map(|n| n.subject).collect();
        assert_eq!(subjects, [day.countries[0].id.into(), day.title_types[0].id.into(), day.people[0].id.into(), day.cities[0].id.into()]);
    }

    #[test]
    fn the_subject_is_stored_but_never_shown() {
        let day = day();
        for news in &day.newsreel {
            let headline = news.headline(&day);
            assert!(!headline.contains(&news.subject.to_string()), "{headline}");
        }
    }

    #[test]
    fn news_is_saved_with_the_id_of_its_subject_and_its_kind() {
        let id = CountryId::from_u128(0x3f2b8c1e_5a47_4d0e_9b6a_7c1d2e8f4a90);
        let news = News::about(id, Event::CountryFounded);
        let json = serde_json::to_string(&news).unwrap();
        assert_eq!(json, r#"{"subject":"3f2b8c1e-5a47-4d0e-9b6a-7c1d2e8f4a90","type":"country_founded"}"#);
        assert_eq!(serde_json::from_str::<News>(&json).unwrap(), news);

        // Whatever else an event carries is saved beside them.
        let city = News::about(CityId::from_u128(7), Event::CityFounded { population: 100, region: Some(5), country: None });
        let json = serde_json::to_string(&city).unwrap();
        assert!(json.contains(r#""type":"city_founded""#) && json.contains(r#""population":100"#), "{json}");
        assert_eq!(serde_json::from_str::<News>(&json).unwrap(), city);
    }

    #[test]
    fn militia_news_reads_naturally() {
        let mut day = day();
        day.found_country("Brevon").unwrap();
        let (aldoria, brevon, port) = (day.countries[0].id, day.countries[1].id, day.cities[0].id);
        let id = crate::ids::MilitiaId::new();
        let headline = |event| News::about(id, event).headline(&day);
        let raised = |name: &str, city, members| Event::MilitiaRaised { name: name.to_owned(), country: aldoria, city, members, cost: vec![Cost::coins(10_000)] };
        assert_eq!(headline(raised("Port Aldor Militia 1-1-1", port, 1_000)), "Aldoria raises Port Aldor Militia 1-1-1 of 1,000 in Port Aldor for 10,000 coins");
        assert_eq!(News::about(aldoria, Event::MilitiaPaid { amount: 450 }).headline(&day), "Aldoria pays 450 coins to keep its militia");
        assert_eq!(News::about(aldoria, Event::MilitiaPaid { amount: 1 }).headline(&day), "Aldoria pays 1 coin to keep its militia");
        let deserted = |name: &str, deserters, remaining| Event::MilitiaDeserted { name: name.to_owned(), country: aldoria, deserters, remaining };
        assert_eq!(headline(deserted("Harbour Guard", 40, 60)), "40 members of Harbour Guard desert for want of pay, leaving 60");
        assert_eq!(headline(deserted("Harbour Guard", 100, 0)), "All 100 members of Harbour Guard desert for want of pay, and it is gone");
        assert_eq!(headline(deserted("", 40, 60)), "40 members of Aldoria's militia desert for want of pay, leaving 60", "news from before militia had names");
        // A transfer names the militia if it is still there, and otherwise says how big it was.
        assert_eq!(headline(Event::MilitiaTransferred { from: aldoria, to: brevon, members: 30 }), "A militia of 30 passes from Aldoria to Brevon");
        assert_eq!(headline(Event::MilitiaMoved { name: "Harbour Guard".to_owned(), points: 84, region: Some(0xC001) }), format!("Harbour Guard marches to {}, spending 84 movement points", day.region_label(0xC001)));
        assert_eq!(headline(Event::MilitiaMoved { name: "Harbour Guard".to_owned(), points: 1, region: None }), "Harbour Guard marches, spending 1 movement point");
        assert_eq!(headline(Event::MilitiaMerged { name: "Harbour Guard".to_owned(), absorbed: "Port Guard".to_owned(), members: 1_000 }), "Port Guard joins Harbour Guard, making 1,000 members");
        assert_eq!(
            headline(Event::MilitiaSplit { name: "Harbour Guard".to_owned(), new_name: "Harbour Guard Detachment".to_owned(), members: 120, remaining: 880 }),
            "Harbour Guard splits off 120 members as Harbour Guard Detachment, leaving 880"
        );
        let disbanded = |members| Event::MilitiaDisbanded { name: "Harbour Guard".to_owned(), members };
        assert_eq!(headline(disbanded(100)), "Harbour Guard is disbanded, and its 100 members disappear");
        assert_eq!(headline(disbanded(1)), "Harbour Guard is disbanded, and its 1 member disappears");
        assert_eq!(headline(Event::MilitiaRenamed { from: "Port Aldor Militia 1-1-1".to_owned(), to: "Harbour Guard".to_owned() }), "Port Aldor Militia 1-1-1 is renamed Harbour Guard");
        // A city that is gone is "a city", and the news survives the militia.
        assert_eq!(headline(raised("", CityId::new(), 5)), "Aldoria raises a militia of 5 in a city for 10,000 coins");
    }
}
