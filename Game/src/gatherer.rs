//! Gatherers: a country's camp where there is a resource to gather, of a kind: a woodcutter in a forest, a
//! stonecutter in stone, and the greater ones, a sawmill for wood and a quarry for stone, which cost much more, in
//! materials too, and provide much more. One can only be put in a region that has some of the resource, only one of the
//! lesser and the greater kind of a resource between them to a region (a woodcutter or a sawmill, a stonecutter or a
//! quarry). It belongs to whichever country owns the region it stands in, which is the country that built it, and which
//! has to own the region to build there; the country that pays for it. It can't be given to another country: only the
//! region changes hands, and the gatherer goes with it.

use std::collections::{HashMap, HashSet};

use macroquad::prelude::*;
use nworld_map::world::{Resource, WaterClass, World};
use serde::{Deserialize, Serialize};

use crate::day::DaySave;
use crate::format;
use crate::ids::{CountryId, GathererId};
use crate::newsreel::{Cost, Event, News};
use crate::transfer::Asset;

/// How dense a resource has to be at a point of the map, from 0 to 255, to count as there, and how many such points a
/// region needs to have for it to count as having the resource at all: a stray speck doesn't make a forest.
const DEPOSIT_DENSITY: u8 = 32;
const DEPOSIT_SAMPLES: usize = 4;

/// What a gatherer gathers, and so what it is called.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum GathererKind {
    /// Gathers wood, from a forest.
    #[default]
    Woodcutter,
    /// Gathers wood, a great deal of it.
    Sawmill,
    /// Gathers stone.
    Stonecutter,
    /// Gathers stone, a great deal of it.
    Quarry,
    /// Gathers iron. There is no lesser one.
    IronMine,
    /// Gathers copper. There is no lesser one.
    CopperMine,
    /// Gathers sulphur. There is no lesser one.
    SulphurPit,
    /// Gathers oil. There is no lesser one.
    OilWell,
}

impl GathererKind {
    /// Every kind, in the order the Buildings menu has them: each of wood and then each of stone, the lesser first, and then
    /// iron, copper, sulphur and oil.
    pub const ALL: [GathererKind; 8] = [
        GathererKind::Woodcutter,
        GathererKind::Sawmill,
        GathererKind::Stonecutter,
        GathererKind::Quarry,
        GathererKind::IronMine,
        GathererKind::CopperMine,
        GathererKind::SulphurPit,
        GathererKind::OilWell,
    ];

    /// What to call one, in lower case: "woodcutter", "sawmill", "stonecutter", "quarry", "iron mine", "copper mine",
    /// "sulphur pit" or "oil well".
    pub fn noun(self) -> &'static str {
        match self {
            Self::Woodcutter => "woodcutter",
            Self::Sawmill => "sawmill",
            Self::Stonecutter => "stonecutter",
            Self::Quarry => "quarry",
            Self::IronMine => "iron mine",
            Self::CopperMine => "copper mine",
            Self::SulphurPit => "sulphur pit",
            Self::OilWell => "oil well",
        }
    }

    /// What to call one with "a" or "an" before it: "a woodcutter", "an iron mine".
    pub fn a_noun(self) -> String {
        let noun = self.noun();
        format!("{} {noun}", if noun.starts_with(['a', 'e', 'i', 'o', 'u']) { "an" } else { "a" })
    }

    /// [`a_noun`](Self::a_noun) to start a sentence: "A woodcutter", "An iron mine".
    pub fn a_noun_capital(self) -> String {
        let text = self.a_noun();
        format!("{}{}", text[..1].to_uppercase(), &text[1..])
    }

    /// What to call several of them: "woodcutters", "sawmills", "stonecutters", "quarries", "iron mines", "copper
    /// mines", "sulphur pits" or "oil wells".
    pub fn plural(self) -> &'static str {
        match self {
            Self::Woodcutter => "woodcutters",
            Self::Sawmill => "sawmills",
            Self::Stonecutter => "stonecutters",
            Self::Quarry => "quarries",
            Self::IronMine => "iron mines",
            Self::CopperMine => "copper mines",
            Self::SulphurPit => "sulphur pits",
            Self::OilWell => "oil wells",
        }
    }

    /// What to call it where a name starts something: "Woodcutter", "Sawmill", "Stonecutter", "Quarry", "Iron mine",
    /// "Copper mine", "Sulphur pit" or "Oil well".
    pub fn label(self) -> &'static str {
        match self {
            Self::Woodcutter => "Woodcutter",
            Self::Sawmill => "Sawmill",
            Self::Stonecutter => "Stonecutter",
            Self::Quarry => "Quarry",
            Self::IronMine => "Iron mine",
            Self::CopperMine => "Copper mine",
            Self::SulphurPit => "Sulphur pit",
            Self::OilWell => "Oil well",
        }
    }

    /// The resource of the map it gathers.
    pub fn resource(self) -> Resource {
        match self {
            Self::Woodcutter | Self::Sawmill => Resource::Forest,
            Self::Stonecutter | Self::Quarry => Resource::Stone,
            Self::IronMine => Resource::Iron,
            Self::CopperMine => Resource::Copper,
            Self::SulphurPit => Resource::Sulphur,
            Self::OilWell => Resource::Oil,
        }
    }

    /// What the resource it gathers is called as it lies on the map: "forest", "stone", "iron", "copper", "sulphur" or
    /// "oil".
    pub fn deposit(self) -> &'static str {
        match self {
            Self::Woodcutter | Self::Sawmill => "forest",
            Self::Stonecutter | Self::Quarry => "stone",
            Self::IronMine => "iron",
            Self::CopperMine => "copper",
            Self::SulphurPit => "sulphur",
            Self::OilWell => "oil",
        }
    }

    /// The kinds that gather the same resource as this one does, the lesser first, of which there can be only one in a
    /// region. Iron, copper, sulphur and oil have the one kind each.
    pub fn same_resource(self) -> &'static [GathererKind] {
        match self {
            Self::Woodcutter | Self::Sawmill => &[Self::Woodcutter, Self::Sawmill],
            Self::Stonecutter | Self::Quarry => &[Self::Stonecutter, Self::Quarry],
            Self::IronMine => &[Self::IronMine],
            Self::CopperMine => &[Self::CopperMine],
            Self::SulphurPit => &[Self::SulphurPit],
            Self::OilWell => &[Self::OilWell],
        }
    }

    /// The kinds that gather the same resource, named for there being only one of them to a region: "woodcutter or
    /// sawmill", or for a kind that is on its own, just "iron mine".
    pub fn same_resource_text(self) -> String {
        self.same_resource().iter().map(|kind| kind.noun()).collect::<Vec<_>>().join(" or ")
    }

    /// Whether it is one of the greater ones, which are built bigger, cost more and provide more: a sawmill, a quarry, and
    /// every kind that has no lesser one.
    pub fn greater(self) -> bool {
        !matches!(self, Self::Woodcutter | Self::Stonecutter)
    }

    /// What the country gets of it: "wood", "stone", "iron", "copper", "sulphur" or "oil".
    pub fn product(self) -> &'static str {
        crate::country::resource_name(self.resource())
    }

    /// What one costs to build, in coins: 1,000 for a woodcutter or a stonecutter, and 10,000 for any other. (The new
    /// kinds cost the same as the greater ones for now, until each has a cost of its own.)
    pub fn cost(self) -> i64 {
        if self.greater() {
            10_000
        } else {
            1_000
        }
    }

    /// What one takes to build besides coins: nothing for a woodcutter or a stonecutter, and 1,000 wood and 1,000 stone for
    /// any other.
    pub fn materials(self) -> &'static [(Resource, u64)] {
        if self.greater() {
            &[(Resource::Forest, 1_000), (Resource::Stone, 1_000)]
        } else {
            &[]
        }
    }

    /// How much of its resource one that is operating provides its country each day: ten times as much from a greater
    /// one, which costs ten times as many coins.
    pub fn daily_yield(self) -> u64 {
        if self.greater() {
            1_000
        } else {
            100
        }
    }

    /// Draws its icon, centered on `c` and about 16 pixels across times `s`, in `color`, with `accent` for what is
    /// marked on it: a pine, a sawblade, a rock, steps of stone, an ingot, a coin, a crystal or a drop.
    pub fn draw_icon(self, c: Vec2, s: f32, color: Color, accent: Color) {
        match self {
            Self::Woodcutter => {
                draw_triangle(vec2(c.x, c.y - 8.0 * s), vec2(c.x - 6.0 * s, c.y + 4.0 * s), vec2(c.x + 6.0 * s, c.y + 4.0 * s), color);
                draw_line(c.x, c.y + 4.0 * s, c.x, c.y + 8.0 * s, 2.0 * s, color);
            }
            Self::Sawmill => {
                draw_poly(c.x, c.y, 8, 8.0 * s, 0.0, color);
                draw_circle(c.x, c.y, 2.6 * s, accent);
                draw_line(c.x - 5.0 * s, c.y - 1.0 * s, c.x + 5.0 * s, c.y + 1.0 * s, 1.2 * s, accent);
            }
            Self::Stonecutter => {
                draw_poly(c.x, c.y + 1.0 * s, 6, 8.0 * s, 0.0, color);
                draw_line(c.x - 3.0 * s, c.y - 4.0 * s, c.x + 1.0 * s, c.y + 1.0 * s, 1.4 * s, accent);
                draw_line(c.x + 1.0 * s, c.y + 1.0 * s, c.x - 1.0 * s, c.y + 6.0 * s, 1.4 * s, accent);
            }
            Self::Quarry => {
                draw_rectangle(c.x - 8.0 * s, c.y + 2.0 * s, 16.0 * s, 5.0 * s, color);
                draw_rectangle(c.x - 5.5 * s, c.y - 3.0 * s, 11.0 * s, 5.0 * s, color);
                draw_rectangle(c.x - 3.0 * s, c.y - 8.0 * s, 6.0 * s, 5.0 * s, color);
                draw_line(c.x - 8.0 * s, c.y + 2.0 * s, c.x + 8.0 * s, c.y + 2.0 * s, 1.0 * s, accent);
                draw_line(c.x - 5.5 * s, c.y - 3.0 * s, c.x + 5.5 * s, c.y - 3.0 * s, 1.0 * s, accent);
            }
            // An ingot: a block wider at the foot than at the top.
            Self::IronMine => {
                draw_triangle(vec2(c.x - 5.0 * s, c.y - 5.0 * s), vec2(c.x + 5.0 * s, c.y - 5.0 * s), vec2(c.x - 8.0 * s, c.y + 6.0 * s), color);
                draw_triangle(vec2(c.x + 5.0 * s, c.y - 5.0 * s), vec2(c.x + 8.0 * s, c.y + 6.0 * s), vec2(c.x - 8.0 * s, c.y + 6.0 * s), color);
                draw_line(c.x - 4.0 * s, c.y - 1.0 * s, c.x + 4.0 * s, c.y - 1.0 * s, 1.4 * s, accent);
            }
            // A coin.
            Self::CopperMine => {
                draw_circle(c.x, c.y, 8.0 * s, color);
                draw_circle(c.x, c.y, 4.6 * s, accent);
                draw_circle(c.x, c.y, 2.4 * s, color);
            }
            // A crystal.
            Self::SulphurPit => {
                draw_poly(c.x, c.y, 4, 9.0 * s, 0.0, color);
                draw_line(c.x, c.y - 9.0 * s, c.x, c.y + 9.0 * s, 1.2 * s, accent);
                draw_line(c.x - 9.0 * s, c.y, c.x + 9.0 * s, c.y, 1.2 * s, accent);
            }
            // A drop.
            Self::OilWell => {
                draw_triangle(vec2(c.x, c.y - 9.0 * s), vec2(c.x - 5.5 * s, c.y + 1.0 * s), vec2(c.x + 5.5 * s, c.y + 1.0 * s), color);
                draw_circle(c.x, c.y + 2.5 * s, 5.5 * s, color);
                draw_circle(c.x - 1.8 * s, c.y + 1.5 * s, 1.4 * s, accent);
            }
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Gatherer {
    pub id: GathererId,
    /// What it gathers. A gatherer saved before there were kinds is a woodcutter.
    #[serde(default)]
    pub kind: GathererKind,
    /// The country it belongs to.
    pub country: CountryId,
    /// Where it stands, in world x and z.
    pub x: f32,
    pub z: f32,
    /// The number of the map region it stands in.
    pub region: u16,
    /// Whether it was operating when last looked at (see [`DaySave::update_gatherer_operation`]), which is for
    /// knowing when that changes. A gatherer starts off not operating.
    #[serde(default)]
    pub operating: bool,
}

/// Which regions of a map have how much of each resource a gatherer gathers: for each kind, the numbers of the regions
/// that have some of it. This takes a pass over the whole map, so it is worked out once and kept.
pub struct Deposits {
    regions: HashMap<GathererKind, HashSet<u16>>,
}

impl Deposits {
    /// The regions of `world` that have a resource of each kind.
    pub fn of(world: &World) -> Self {
        let regions = GathererKind::ALL.into_iter().map(|kind| (kind, regions_with(world, kind.resource()))).collect();
        Self { regions }
    }

    /// Whether the region numbered `region` has the resource that `kind` gathers.
    pub fn has(&self, kind: GathererKind, region: u16) -> bool {
        self.regions.get(&kind).is_some_and(|regions| regions.contains(&region))
    }
}

/// The numbers of the regions of `world` that have `resource` in them: enough of it, at a density that counts.
fn regions_with(world: &World, resource: Resource) -> HashSet<u16> {
    let mut counts: HashMap<u16, usize> = HashMap::new();
    for (region, density) in world.regions().iter().zip(world.resource(resource)) {
        if *region != 0 && *density >= DEPOSIT_DENSITY {
            *counts.entry(*region).or_default() += 1;
        }
    }
    counts.into_iter().filter(|(_, samples)| *samples >= DEPOSIT_SAMPLES).map(|(region, _)| region).collect()
}

/// The number of the region that a gatherer of `kind` put at `at` would be in, or why it can't go there: it has to be
/// on the map, on dry land, and in a region that has its resource, according to `deposits`.
pub fn site_region(world: &World, deposits: &Deposits, kind: GathererKind, at: Vec2) -> Result<u16, String> {
    let half = world.half_extents();
    if at.x.abs() > half.x || at.y.abs() > half.y {
        return Err("That is off the map.".to_owned());
    }
    if world.water_class(at.x, at.y) != WaterClass::Dry {
        return Err(format!("{} has to stand on dry land.", kind.a_noun_capital()));
    }
    let region = world.region_at(at.x, at.y).ok_or_else(|| format!("This map has no regions to put {} in.", kind.a_noun()))?.raw();
    if !deposits.has(kind, region) {
        return Err(format!("{} can only go in a region that has some {}.", kind.a_noun_capital(), kind.deposit()));
    }
    Ok(region)
}

impl DaySave {
    /// The gatherer with this id.
    pub fn gatherer(&self, id: GathererId) -> Option<&Gatherer> {
        self.gatherers.iter().find(|g| g.id == id)
    }

    /// The gatherer of `resource` in the map region numbered `region`, of either kind that gathers it, if there is one.
    pub fn gatherer_in(&self, region: u16, resource: Resource) -> Option<&Gatherer> {
        self.gatherers.iter().find(|g| g.region == region && g.kind.resource() == resource)
    }

    /// Why a gatherer of `kind` can't be built now, or `None` if one can: the country being played has to be there and
    /// able to pay for it, in coins, and for a greater one in wood and stone too.
    pub fn gatherer_problem(&self, kind: GathererKind) -> Option<String> {
        match self.player_country() {
            None => Some(format!("There is no country yet to own {}.", kind.a_noun())),
            Some(country) if country.money < kind.cost() => {
                Some(format!("Not enough coin: {} costs {} coins, and {} has {}.", kind.a_noun(), format::money(kind.cost()), country.name, format::money(country.money)))
            }
            // Then the wood and the stone a greater one takes, whichever is short first.
            Some(country) => kind.materials().iter().find(|(resource, needed)| country.stock.get(*resource) < *needed).map(|(resource, needed)| {
                let name = crate::country::resource_name(*resource);
                format!("Not enough {name}: {} needs {} {name} as well as its coins, and {} has {}.", kind.a_noun(), format::thousands_u64(*needed), country.name, format::thousands_u64(country.stock.get(*resource)))
            }),
        }
    }

    /// Why the country being played can't have a gatherer of `kind` in the region numbered `region`, or `None` if it
    /// can: the region has to be the country's, and can't have one in it already that gathers the same resource, of
    /// either kind: a sawmill can't go where there is a woodcutter, nor a woodcutter where there is a sawmill.
    pub fn gatherer_region_problem(&self, region: u16, kind: GathererKind) -> Option<String> {
        let country = self.player_country()?.id;
        self.placement_region_problem(country, Some(region)).or_else(|| self.gatherer_in(region, kind.resource()).map(|there| format!("There is already {} in this region.", there.kind.a_noun())))
    }

    /// Builds a gatherer of `kind` at (`x`, `z`) in the region numbered `region` for the country being played, which
    /// pays for it (in wood and stone as well, for a greater one), and reports it in the newsreel. Whether the place is
    /// fit for one (see [`site_region`]) is for the
    /// caller to say. An error says why it can't be done.
    pub fn build_gatherer(&mut self, kind: GathererKind, x: f32, z: f32, region: u16) -> Result<(), String> {
        if let Some(problem) = self.gatherer_problem(kind) {
            return Err(problem);
        }
        if !x.is_finite() || !z.is_finite() {
            return Err("That is not a place on the map.".to_owned());
        }
        if let Some(problem) = self.gatherer_region_problem(region, kind) {
            return Err(problem);
        }
        let country = self.player_country().ok_or_else(|| format!("There is no country yet to own {}.", kind.a_noun()))?.id;
        self.add_money(country, -kind.cost())?;
        for (resource, needed) in kind.materials() {
            self.add_resource(country, *resource, -(*needed as i64))?;
        }
        let id = GathererId::new();
        self.gatherers.push(Gatherer { id, kind, country, x, z, region, operating: false });
        let mut cost = vec![Cost::coins(kind.cost())];
        cost.extend(kind.materials().iter().map(|(resource, needed)| Cost { asset: Asset::of(*resource), amount: *needed }));
        self.newsreel.push(News::about(id, Event::GathererBuilt { kind, country: Some(country), region: Some(region), cost }));
        Ok(())
    }

    /// For tests: claims the region numbered `region` for the country being played if nobody owns it, and then builds a gatherer
    /// there.
    #[cfg(test)]
    pub fn build_gatherer_claiming(&mut self, kind: GathererKind, x: f32, z: f32, region: u16) -> Result<(), String> {
        if self.region_owner(region).is_none() {
            self.claim_region(region)?;
        }
        self.build_gatherer(kind, x, z, region)
    }

    /// Destroys the gatherer with this id, which costs nothing and pays nothing back, and reports it in the
    /// newsreel. An error says why it can't be done.
    pub fn destroy_gatherer(&mut self, id: GathererId) -> Result<(), String> {
        let Gatherer { kind, country, region, .. } = *self.gatherer(id).ok_or("There is no such gatherer to destroy.")?;
        self.gatherers.retain(|g| g.id != id);
        self.newsreel.push(News::about(id, Event::GathererDestroyed { country, region, kind }));
        Ok(())
    }

    /// What to call the map region numbered `region` in a sentence: its name if it has one, and otherwise "region" and
    /// its number.
    pub fn region_label(&self, region: u16) -> String {
        self.region_name(region).map_or_else(|| format!("region {}", region & 0x3FFF), str::to_owned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::Date;
    
    use nworld_map::world::SEA_LEVEL;

    fn day_with_money(money: i64) -> DaySave {
        let mut day = DaySave::new(Date::FIRST, "Assets/map.nworld");
        day.found_country("Aldoria").unwrap();
        let id = day.countries[0].id;
        let have = day.countries[0].money;
        day.add_money(id, money - have).unwrap();
        day
    }

    /// A dry world of two regions, 1 in the west half and 2 in the east, with `forest` painted at full density on
    /// the samples for which it is true.
    fn world_with_forest(forest: impl Fn(usize, usize) -> bool) -> World {
        let mut world = World::new(512.0, 512.0);
        let (nx, nz) = world.samples();
        world.set_terrain(vec![10.0; nx * nz], vec![SEA_LEVEL; nx * nz]);
        world.set_regions((0..nz).flat_map(|_| (0..nx).map(|i| if i < nx / 2 { 1 } else { 2 })).collect());
        for j in 0..nz {
            for i in 0..nx {
                if forest(i, j) {
                    world.resource_mut(Resource::Forest)[j * nx + i] = 255;
                }
            }
        }
        world
    }

    #[test]
    fn a_region_has_forest_when_enough_of_it_is_dense_and_a_speck_is_not_enough() {
        // A patch in the west, and a single stray sample in the east.
        let world = world_with_forest(|i, j| (10..20).contains(&i) && (10..20).contains(&j) || (i, j) == (100, 100));
        assert_eq!(regions_with(&world, Resource::Forest), HashSet::from([1]));
        assert_eq!(Deposits::of(&world).has(GathererKind::Woodcutter, 1), true);
        assert!(!Deposits::of(&world).has(GathererKind::Woodcutter, 2) && !Deposits::of(&world).has(GathererKind::Stonecutter, 1), "no stone anywhere");
        // A patch only faintly painted doesn't count either.
        let mut faint = world_with_forest(|_, _| false);
        faint.resource_mut(Resource::Forest).fill(DEPOSIT_DENSITY - 1);
        assert!(regions_with(&faint, Resource::Forest).is_empty());
        // Nor does a map with no regions.
        assert!(regions_with(&World::new(512.0, 512.0), Resource::Forest).is_empty());
    }

    #[test]
    fn a_gatherer_goes_on_dry_land_in_a_forest_region_on_the_map() {
        let world = world_with_forest(|i, j| (10..20).contains(&i) && (10..20).contains(&j));
        let forest = Deposits::of(&world);
        let west = vec2(-100.0, 0.0);
        assert_eq!(site_region(&world, &forest, GathererKind::Woodcutter, west), Ok(1));
        // The other region has no forest, so is refused, whether or not there is forest on the spot itself.
        assert!(site_region(&world, &forest, GathererKind::Woodcutter, vec2(100.0, 0.0)).unwrap_err().contains("some forest"));
        assert!(site_region(&world, &forest, GathererKind::Woodcutter, vec2(9_999.0, 0.0)).unwrap_err().contains("off the map"));
        // Water is no place for one, even in a forest region.
        let mut flooded = world_with_forest(|i, j| (10..20).contains(&i) && (10..20).contains(&j));
        flooded.water_mut().fill(20.0);
        assert!(site_region(&flooded, &forest, GathererKind::Woodcutter, west).unwrap_err().contains("dry land"));
        // Nor is a map not divided into regions.
        let plain = World::new(512.0, 512.0);
        assert!(site_region(&plain, &forest, GathererKind::Woodcutter, Vec2::ZERO).is_err());
    }

    #[test]
    fn a_gatherer_costs_1000_coins_from_the_country_being_played_and_belongs_to_it() {
        let mut day = day_with_money(2_500);
        day.found_country("Brevon").unwrap();
        day.build_gatherer_claiming(GathererKind::Woodcutter, -100.0, 20.0, 7).unwrap();
        assert_eq!(day.gatherers.len(), 1);
        let aldoria = day.countries[0].id;
        let cutter = day.gatherers[0];
        assert_eq!((cutter.country, cutter.x, cutter.z, cutter.region), (aldoria, -100.0, 20.0, 7));
        assert_eq!(day.countries[0].money, 1_500);
        assert_eq!(day.countries[1].money, crate::country::STARTING_MONEY, "only the country being played pays");
        assert_eq!(day.gatherer(cutter.id), Some(&cutter));
        assert_eq!(day.gatherer_in(7, Resource::Forest), Some(&cutter));
        assert_eq!(day.gatherer_in(7, Resource::Stone), None, "it is not stone that it gathers");
        assert_eq!(day.gatherer_in(8, Resource::Forest), None);
    }

    #[test]
    fn building_a_gatherer_is_in_the_newsreel_with_the_gatherer_as_its_subject() {
        let mut day = day_with_money(2_000);
        day.claim_region(0xC007).unwrap();
        let before = day.newsreel.len();
        day.build_gatherer(GathererKind::Woodcutter, -100.0, 20.0, 0xC007).unwrap();
        let id = day.gatherers[0].id;
        assert_eq!(day.newsreel.len(), before + 1);
        let news = day.newsreel.last().unwrap();
        let aldoria = day.countries[0].id;
        assert_eq!(*news, News::about(id, Event::GathererBuilt { kind: GathererKind::Woodcutter, country: Some(aldoria), region: Some(0xC007), cost: vec![Cost::coins(1_000)] }));
        assert_eq!(news.headline(&day), "Aldoria builds a woodcutter in region 7 for 1,000 coins");
        let news = day.newsreel.last().unwrap().clone();
        day.set_region_name(0xC007, "The Greenwood").unwrap();
        assert_eq!(news.headline(&day), "Aldoria builds a woodcutter in The Greenwood for 1,000 coins");
        // It is saved with its kind, and the subject's id is never in the words.
        let json = serde_json::to_string(&news).unwrap();
        assert!(json.contains(r#""type":"gatherer_built""#), "{json}");
        assert!(!news.headline(&day).contains(&id.to_string()));
        // News from before the country, the region and the cost were kept reads on without them, and a gatherer that
        // can't be built is not reported.
        let old: News = serde_json::from_str(&format!(r#"{{"subject":"{id}","type":"woodcutter_built"}}"#)).unwrap();
        assert_eq!(old.headline(&day), "Aldoria builds a woodcutter in The Greenwood", "from the gatherer, which is there");
        assert_eq!(News::about(GathererId::new(), Event::GathererBuilt { kind: GathererKind::Woodcutter, country: None, region: None, cost: Vec::new() }).headline(&day), "A woodcutter is built");
        let count = day.newsreel.len();
        assert!(day.build_gatherer_claiming(GathererKind::Woodcutter, 0.0, 0.0, 0xC007).is_err());
        assert_eq!(day.newsreel.len(), count);
    }

    #[test]
    fn a_gatherer_belongs_to_whoever_owns_its_region_and_goes_with_it() {
        let mut day = day_with_money(5_000);
        day.found_country("Brevon").unwrap();
        let (aldoria, brevon) = (day.countries[0].id, day.countries[1].id);
        day.found_city("Port Aldor", 0.0, 0.0, Some(0xC001)).unwrap();
        day.build_gatherer(GathererKind::Woodcutter, 10.0, 0.0, 0xC001).unwrap();
        let id = day.gatherers[0].id;
        assert_eq!((day.gatherer(id).unwrap().country, day.region_owner(0xC001)), (aldoria, Some(aldoria)));

        // The city, and its region with it, is given to Brevon, and what stands in the region goes along.
        let city = day.cities[0].id;
        day.set_city_country(city, brevon).unwrap();
        assert_eq!(day.region_owner(0xC001), Some(brevon));
        assert_eq!(day.gatherer(id).unwrap().country, brevon, "the gatherer follows the region");
        assert_eq!((day.gatherer(id).unwrap().x, day.gatherer(id).unwrap().region), (10.0, 0xC001), "and stays where it is");
        // Which country it is changes which cities it has to reach, and Brevon's city is in its region.
        assert!(day.cut_off_gatherers().is_empty());
    }
    #[test]
    fn destroying_a_gatherer_is_in_the_newsreel_and_old_news_of_a_transfer_still_reads() {
        let mut day = day_with_money(5_000);
        day.found_country("Brevon").unwrap();
        let (aldoria, brevon) = (day.countries[0].id, day.countries[1].id);
        day.build_gatherer_claiming(GathererKind::Woodcutter, 0.0, 0.0, 0xC001).unwrap();
        let id = day.gatherers[0].id;

        // Destroyed, it is gone, and the news still says whose it was and where.
        day.destroy_gatherer(id).unwrap();
        let news = day.newsreel.last().unwrap();
        assert_eq!(*news, News::about(id, Event::GathererDestroyed { country: aldoria, region: 0xC001, kind: GathererKind::Woodcutter }));
        assert_eq!(news.headline(&day), "Aldoria's woodcutter in region 1 is destroyed");
        let json = serde_json::to_string(news).unwrap();
        assert!(json.contains(r#""type":"gatherer_destroyed""#) && json.contains(r#""region":49153"#), "{json}");
        assert_eq!(serde_json::from_str::<News>(&json).unwrap(), *news);
        let count = day.newsreel.len();
        assert!(day.destroy_gatherer(id).is_err());
        assert_eq!(day.newsreel.len(), count, "one that isn't there is not news");

        // A gatherer could once be given to another country, and a game saved then still reads its news.
        let old = News::about(id, Event::GathererTransferred { from: aldoria, to: brevon, kind: GathererKind::Woodcutter, region: Some(0xC001) });
        assert_eq!(old.headline(&day), "The woodcutter in region 1 is transferred from Aldoria to Brevon");
        assert!(serde_json::to_string(&old).unwrap().contains(r#""type":"gatherer_transferred""#));
        assert_eq!(News::about(id, Event::GathererTransferred { from: aldoria, to: brevon, kind: GathererKind::Woodcutter, region: None }).headline(&day), "A woodcutter is transferred from Aldoria to Brevon");
    }
    #[test]
    fn a_gatherer_destroyed_is_gone_for_nothing_and_its_region_has_room_for_another() {
        let mut day = day_with_money(5_000);
        day.build_gatherer_claiming(GathererKind::Woodcutter, 0.0, 0.0, 0xC001).unwrap();
        let id = day.gatherers[0].id;
        let paid = day.countries[0].money;
        day.destroy_gatherer(id).unwrap();
        assert!(day.gatherers.is_empty() && day.gatherer(id).is_none() && day.gatherer_in(0xC001, Resource::Forest).is_none());
        assert_eq!(day.countries[0].money, paid, "nothing is paid back");
        assert!(day.destroy_gatherer(id).unwrap_err().contains("no such gatherer"));
        assert!(day.cut_off_gatherers().is_empty());
        day.build_gatherer_claiming(GathererKind::Woodcutter, 3.0, 3.0, 0xC001).unwrap();
        assert_eq!(day.gatherers.len(), 1);
    }

    #[test]
    fn each_kind_needs_what_it_gathers_in_the_region_forest_for_a_woodcutter_and_stone_for_a_stonecutter() {
        // Forest in the west region (1) and stone in the east (2), both a patch of full density.
        let mut world = world_with_forest(|i, j| (10..20).contains(&i) && (10..20).contains(&j));
        let (nx, _) = world.samples();
        for j in 10..20 {
            for i in (nx - 20)..(nx - 10) {
                world.resource_mut(Resource::Stone)[j * nx + i] = 255;
            }
        }
        let deposits = Deposits::of(&world);
        assert!(deposits.has(GathererKind::Woodcutter, 1) && !deposits.has(GathererKind::Woodcutter, 2));
        assert!(deposits.has(GathererKind::Stonecutter, 2) && !deposits.has(GathererKind::Stonecutter, 1));
        let (west, east) = (vec2(-100.0, 0.0), vec2(100.0, 0.0));
        assert_eq!(site_region(&world, &deposits, GathererKind::Woodcutter, west), Ok(1));
        assert_eq!(site_region(&world, &deposits, GathererKind::Stonecutter, east), Ok(2));
        assert_eq!(site_region(&world, &deposits, GathererKind::Stonecutter, west).unwrap_err(), "A stonecutter can only go in a region that has some stone.");
        assert_eq!(site_region(&world, &deposits, GathererKind::Woodcutter, east).unwrap_err(), "A woodcutter can only go in a region that has some forest.");
    }

    #[test]
    fn a_region_has_room_for_one_of_each_kind_and_a_stonecutter_costs_the_same_and_is_named_for_what_it_is() {
        let mut day = day_with_money(5_000);
        day.build_gatherer_claiming(GathererKind::Woodcutter, 0.0, 0.0, 0xC001).unwrap();
        // A stonecutter can share the region with the woodcutter, but a second of either kind can't.
        assert_eq!(day.gatherer_region_problem(0xC001, GathererKind::Stonecutter), None);
        day.build_gatherer_claiming(GathererKind::Stonecutter, 20.0, 0.0, 0xC001).unwrap();
        assert_eq!(day.countries[0].money, 3_000, "each costs 1,000");
        assert_eq!(day.gatherers.iter().map(|g| g.kind).collect::<Vec<_>>(), [GathererKind::Woodcutter, GathererKind::Stonecutter]);
        assert_eq!(day.gatherer_in(0xC001, Resource::Stone).unwrap().kind, GathererKind::Stonecutter);
        assert_eq!(day.gatherer_region_problem(0xC001, GathererKind::Stonecutter).unwrap(), "There is already a stonecutter in this region.");
        let before = day.clone();
        assert!(day.build_gatherer_claiming(GathererKind::Stonecutter, 30.0, 0.0, 0xC001).unwrap_err().contains("already a stonecutter"));
        assert!(day.build_gatherer_claiming(GathererKind::Woodcutter, 30.0, 0.0, 0xC001).unwrap_err().contains("already a woodcutter"));
        assert_eq!(day, before);
        // Short of coin, it says it is a stonecutter that costs it.
        let mut poor = day_with_money(999);
        assert!(poor.build_gatherer(GathererKind::Stonecutter, 0.0, 0.0, 0xC001).unwrap_err().contains("a stonecutter costs 1,000"));
        assert!(DaySave::new(Date::FIRST, "x").gatherer_problem(GathererKind::Stonecutter).unwrap().contains("own a stonecutter"));
        // Its news and the countries' lists name it too.
        assert_eq!(day.newsreel.last().unwrap().headline(&day), "Aldoria builds a stonecutter in region 1 for 1,000 coins");
        assert_eq!(day.delete_problem(day.countries[0].id).unwrap(), "Aldoria owns 1 woodcutter, 1 stonecutter and 1 region, so it can't be deleted.");
        assert!(serde_json::to_string(day.newsreel.last().unwrap()).unwrap().contains(r#""type":"gatherer_built","kind":"stonecutter""#));
    }

    #[test]
    fn a_sawmill_and_a_quarry_need_the_same_deposits_as_the_lesser_ones_and_cost_coin_wood_and_stone() {
        let mut day = day_with_money(20_000);
        let id = day.countries[0].id;
        // Rich in coin and not in materials: it is the wood that is asked for, and then the stone.
        day.claim_region(0xC001).unwrap();
        let before = day.clone();
        for kind in [GathererKind::Sawmill, GathererKind::Quarry] {
            assert_eq!(day.gatherer_problem(kind).unwrap(), format!("Not enough wood: a {} needs 1,000 wood as well as its coins, and Aldoria has 0.", kind.noun()));
            assert!(day.build_gatherer_claiming(kind, 0.0, 0.0, 0xC001).unwrap_err().contains("Not enough wood"));
        }
        assert_eq!(day, before, "nothing is taken when it can't be done");
        day.add_resource(id, Resource::Forest, 1_000).unwrap();
        assert_eq!(day.gatherer_problem(GathererKind::Quarry).unwrap(), "Not enough stone: a quarry needs 1,000 stone as well as its coins, and Aldoria has 0.");
        day.add_resource(id, Resource::Stone, 1_500).unwrap();
        assert_eq!(day.gatherer_problem(GathererKind::Sawmill), None);
        // A lesser one asks for none of it, and still only the coin.
        assert_eq!(day.gatherer_problem(GathererKind::Woodcutter), None);

        day.build_gatherer_claiming(GathererKind::Sawmill, 0.0, 0.0, 0xC001).unwrap();
        let country = &day.countries[0];
        assert_eq!((country.money, country.stock.wood, country.stock.stone), (10_000, 0, 500));
        assert_eq!(day.newsreel.last().unwrap().headline(&day), "Aldoria builds a sawmill in region 1 for 10,000 coins, 1,000 wood and 1,000 stone");
        // The materials are used up, so a quarry can't follow it until there is more.
        assert!(day.build_gatherer_claiming(GathererKind::Quarry, 20.0, 0.0, 0xC001).unwrap_err().contains("Not enough wood"));
        // A woodcutter can't join the sawmill, which already gathers the wood of the region, and neither can a second sawmill.
        assert_eq!(day.gatherer_region_problem(0xC001, GathererKind::Woodcutter).unwrap(), "There is already a sawmill in this region.");
        assert!(day.build_gatherer_claiming(GathererKind::Woodcutter, 30.0, 0.0, 0xC001).unwrap_err().contains("already a sawmill"));
        assert_eq!(day.gatherer_region_problem(0xC001, GathererKind::Sawmill).unwrap(), "There is already a sawmill in this region.");
        // It takes its country's money: the coin is 10,000 and not 1,000.
        let mut poor = day_with_money(9_999);
        poor.add_resource(poor.countries[0].id, Resource::Forest, 1_000).unwrap();
        poor.add_resource(poor.countries[0].id, Resource::Stone, 1_000).unwrap();
        assert!(poor.build_gatherer(GathererKind::Quarry, 0.0, 0.0, 0xC001).unwrap_err().contains("a quarry costs 10,000"));
        assert_eq!(poor.gatherer_problem(GathererKind::Woodcutter), None, "but it can pay for a woodcutter");
    }

    #[test]
    fn a_region_has_one_gatherer_of_each_resource_whichever_kind_the_lesser_or_the_greater() {
        let mut day = day_with_money(100_000);
        let id = day.countries[0].id;
        day.add_resource(id, Resource::Forest, 10_000).unwrap();
        day.add_resource(id, Resource::Stone, 10_000).unwrap();
        for kind in GathererKind::ALL {
            let same = kind.same_resource();
            assert!(same.contains(&kind) && same.iter().all(|other| other.resource() == kind.resource()));
            match same {
                [lesser, greater] => assert!(!lesser.greater() && greater.greater()),
                [only] => assert!(only.greater(), "a kind on its own is a greater one"),
                _ => panic!("{kind:?} is neither on its own nor one of a pair"),
            }
        }
        // Nothing is in region 1, which is Aldoria's: all of them can go. A woodcutter goes in, and then neither it nor a sawmill can.
        day.claim_region(1).unwrap();
        assert!(GathererKind::ALL.iter().all(|kind| day.gatherer_region_problem(1, *kind).is_none()));
        day.build_gatherer_claiming(GathererKind::Woodcutter, 0.0, 0.0, 1).unwrap();
        assert_eq!(day.gatherer_region_problem(1, GathererKind::Woodcutter).unwrap(), "There is already a woodcutter in this region.");
        assert_eq!(day.gatherer_region_problem(1, GathererKind::Sawmill).unwrap(), "There is already a woodcutter in this region.");
        let before = day.clone();
        assert!(day.build_gatherer_claiming(GathererKind::Sawmill, 10.0, 0.0, 1).unwrap_err().contains("already a woodcutter"));
        assert_eq!(day, before, "and nothing is paid for it");
        // The stone of the region is another matter: either stone kind can still go in, and then the other can't.
        assert_eq!((day.gatherer_region_problem(1, GathererKind::Stonecutter), day.gatherer_region_problem(1, GathererKind::Quarry)), (None, None));
        day.build_gatherer_claiming(GathererKind::Quarry, 20.0, 0.0, 1).unwrap();
        assert_eq!(day.gatherer_region_problem(1, GathererKind::Stonecutter).unwrap(), "There is already a quarry in this region.");
        assert_eq!(day.gatherer_region_problem(1, GathererKind::Quarry).unwrap(), "There is already a quarry in this region.");
        // Other regions are free once they are hers, and once one is destroyed its resource is free again in its region.
        day.claim_region(2).unwrap();
        assert!(GathererKind::ALL.iter().all(|kind| day.gatherer_region_problem(2, *kind).is_none()));
        let woodcutter = day.gatherers[0].id;
        day.destroy_gatherer(woodcutter).unwrap();
        assert_eq!(day.gatherer_region_problem(1, GathererKind::Sawmill), None);
        day.build_gatherer_claiming(GathererKind::Sawmill, 0.0, 0.0, 1).unwrap();
        assert_eq!(day.gatherers.iter().map(|g| g.kind).collect::<Vec<_>>(), [GathererKind::Quarry, GathererKind::Sawmill]);
    }

    #[test]
    fn what_each_kind_gathers_and_is_called() {
        assert_eq!((GathererKind::Woodcutter.noun(), GathererKind::Stonecutter.noun()), ("woodcutter", "stonecutter"));
        assert_eq!((GathererKind::Woodcutter.label(), GathererKind::Stonecutter.label()), ("Woodcutter", "Stonecutter"));
        assert_eq!((GathererKind::Woodcutter.resource(), GathererKind::Stonecutter.resource()), (Resource::Forest, Resource::Stone));
        assert_eq!((GathererKind::Woodcutter.product(), GathererKind::Stonecutter.product()), ("wood", "stone"));
        assert_eq!((GathererKind::Woodcutter.deposit(), GathererKind::Stonecutter.deposit()), ("forest", "stone"));
        assert_eq!(GathererKind::ALL[..4], [GathererKind::Woodcutter, GathererKind::Sawmill, GathererKind::Stonecutter, GathererKind::Quarry]);
        assert_eq!((GathererKind::Sawmill.noun(), GathererKind::Quarry.noun()), ("sawmill", "quarry"));
        assert_eq!((GathererKind::Sawmill.plural(), GathererKind::Quarry.plural()), ("sawmills", "quarries"));
        assert_eq!((GathererKind::Sawmill.label(), GathererKind::Quarry.label()), ("Sawmill", "Quarry"));
        assert_eq!((GathererKind::Sawmill.resource(), GathererKind::Quarry.resource()), (Resource::Forest, Resource::Stone));
        assert_eq!((GathererKind::Sawmill.deposit(), GathererKind::Quarry.deposit()), ("forest", "stone"));
        assert_eq!((GathererKind::Sawmill.product(), GathererKind::Quarry.product()), ("wood", "stone"));
        // The lesser ones cost 1,000 coins and nothing else and give 100 a day; the greater ones, ten times the coin, a
        // thousand each of wood and stone, and ten times the yield.
        for kind in [GathererKind::Woodcutter, GathererKind::Stonecutter] {
            assert!(!kind.greater() && kind.cost() == 1_000 && kind.daily_yield() == 100 && kind.materials().is_empty());
        }
        for kind in [GathererKind::Sawmill, GathererKind::Quarry] {
            assert!(kind.greater() && kind.cost() == 10_000 && kind.daily_yield() == 1_000);
            assert_eq!(kind.materials(), &[(Resource::Forest, 1_000), (Resource::Stone, 1_000)]);
        }
        assert_eq!(GathererKind::default(), GathererKind::Woodcutter);
    }

    #[test]
    fn iron_copper_sulphur_and_oil_each_have_one_kind_of_gatherer_that_costs_what_a_greater_one_does() {
        let kinds = [GathererKind::IronMine, GathererKind::CopperMine, GathererKind::SulphurPit, GathererKind::OilWell];
        assert_eq!(GathererKind::ALL[4..], kinds);
        let resources = [Resource::Iron, Resource::Copper, Resource::Sulphur, Resource::Oil];
        let nouns = ["iron mine", "copper mine", "sulphur pit", "oil well"];
        let plurals = ["iron mines", "copper mines", "sulphur pits", "oil wells"];
        let products = ["iron", "copper", "sulphur", "oil"];
        for (n, kind) in kinds.into_iter().enumerate() {
            assert_eq!((kind.resource(), kind.noun(), kind.plural(), kind.product(), kind.deposit()), (resources[n], nouns[n], plurals[n], products[n], products[n]));
            assert_eq!(kind.same_resource(), [kind], "there is no lesser one");
            assert_eq!(kind.same_resource_text(), nouns[n]);
            // All cost the same for now: 10,000 coins and a thousand each of wood and stone, and give 1,000 a day.
            assert!(kind.greater() && kind.cost() == 10_000 && kind.daily_yield() == 1_000);
            assert_eq!(kind.materials(), &[(Resource::Forest, 1_000), (Resource::Stone, 1_000)]);
        }
        assert_eq!((GathererKind::IronMine.a_noun(), GathererKind::OilWell.a_noun(), GathererKind::CopperMine.a_noun()), ("an iron mine".to_owned(), "an oil well".to_owned(), "a copper mine".to_owned()));
        assert_eq!(GathererKind::IronMine.a_noun_capital(), "An iron mine");
        assert_eq!(GathererKind::Woodcutter.a_noun_capital(), "A woodcutter");
        assert_eq!(GathererKind::Woodcutter.same_resource_text(), "woodcutter or sawmill");
    }

    #[test]
    fn an_iron_mine_is_built_and_named_with_an_an_and_a_second_resource_can_share_its_region() {
        let mut day = day_with_money(20_000);
        let id = day.countries[0].id;
        day.add_resource(id, Resource::Forest, 2_000).unwrap();
        day.add_resource(id, Resource::Stone, 2_000).unwrap();
        day.build_gatherer_claiming(GathererKind::IronMine, 0.0, 0.0, 1).unwrap();
        assert_eq!(day.newsreel.last().unwrap().headline(&day), "Aldoria builds an iron mine in region 1 for 10,000 coins, 1,000 wood and 1,000 stone");
        assert_eq!(day.gatherer_region_problem(1, GathererKind::IronMine).unwrap(), "There is already an iron mine in this region.");
        assert_eq!(day.gatherer_region_problem(1, GathererKind::OilWell), None, "another resource is another matter");
        assert_eq!(day.delete_problem(id).unwrap(), "Aldoria owns 1 iron mine and 1 region, so it can't be deleted.");
        assert!(serde_json::to_string(day.newsreel.last().unwrap()).unwrap().contains(r#""kind":"iron_mine""#));
        // Short of coin or of materials it says what an iron mine takes.
        let poor = day_with_money(500);
        assert_eq!(poor.gatherer_problem(GathererKind::OilWell).unwrap(), "Not enough coin: an oil well costs 10,000 coins, and Aldoria has 500.");
    }

    #[test]
    fn a_country_can_only_build_a_gatherer_in_a_region_it_owns() {
        let mut day = day_with_money(5_000);
        day.found_country("Brevon").unwrap();
        let (aldoria, brevon) = (day.countries[0].id, day.countries[1].id);
        // Nobody's region: it has to be claimed first, and nothing is taken for asking.
        let before = day.clone();
        let err = day.build_gatherer(GathererKind::Woodcutter, 0.0, 0.0, 0xC001).unwrap_err();
        assert!(err.contains("Aldoria doesn't own this region") && err.contains("claim it first"), "{err}");
        assert_eq!(day, before);
        assert!(day.gatherer_region_problem(0xC001, GathererKind::Woodcutter).is_some());
        day.claim_region(0xC001).unwrap();
        assert_eq!(day.gatherer_region_problem(0xC001, GathererKind::Woodcutter), None);
        day.build_gatherer(GathererKind::Woodcutter, 0.0, 0.0, 0xC001).unwrap();
        assert_eq!(day.gatherers[0].country, aldoria);
        // Claimed, it is hers, so Brevon can't build in it or take it, and she can't give it away.
        day.select_country(brevon).unwrap();
        day.add_money(brevon, 5_000).unwrap();
        assert!(day.build_gatherer(GathererKind::Stonecutter, 20.0, 0.0, 0xC001).unwrap_err().contains("belongs to Aldoria"));
        assert!(day.claim_region(0xC001).unwrap_err().contains("has to stay with Aldoria"));
        // Destroying the gatherer leaves her the region she claimed.
        let cutter = day.gatherers[0].id;
        day.destroy_gatherer(cutter).unwrap();
        assert_eq!(day.region_owner(0xC001), Some(aldoria));
    }
    #[test]
    fn a_game_saved_when_gatherers_were_all_woodcutters_still_loads() {
        let mut day = day_with_money(5_000);
        day.build_gatherer_claiming(GathererKind::Woodcutter, 0.0, 0.0, 0xC001).unwrap();
        day.gatherers[0].operating = true;
        day.newsreel.push(News::about(day.countries[0].id, Event::Gathered { amount: 100, kind: GathererKind::Woodcutter }));
        let id = day.gatherers[0].id;
        let mut old = serde_json::to_value(&day).unwrap();
        // The list was called "woodcutters", and a woodcutter had no kind.
        let list = old.as_object_mut().unwrap().remove("gatherers").unwrap();
        old["woodcutters"] = list;
        old["woodcutters"][0].as_object_mut().unwrap().remove("kind");
        // And the news had names of its own, and nothing of kinds.
        for news in old["newsreel"].as_array_mut().unwrap() {
            for (new, was) in [("gatherer_built", "woodcutter_built"), ("gathered", "wood_gathered")] {
                if news["type"] == new {
                    news["type"] = was.into();
                    news.as_object_mut().unwrap().remove("kind");
                }
            }
        }
        let loaded: DaySave = serde_json::from_value(old).unwrap();
        assert_eq!(loaded.gatherers.len(), 1);
        assert_eq!((loaded.gatherers[0].id, loaded.gatherers[0].kind, loaded.gatherers[0].operating), (id, GathererKind::Woodcutter, true));
        assert_eq!(loaded, day, "nothing is lost");
    }

    #[test]
    fn a_gatherer_cannot_be_built_without_the_coin_and_says_why() {
        let mut day = day_with_money(999);
        day.claim_region(1).unwrap();
        let before = day.clone();
        let error = day.build_gatherer(GathererKind::Woodcutter, 0.0, 0.0, 1).unwrap_err();
        assert!(error.contains("Not enough coin") && error.contains("1,000") && error.contains("Aldoria") && error.contains("999"), "{error}");
        assert_eq!(day, before);
        assert!(day.gatherer_problem(GathererKind::Woodcutter).is_some());
        day.add_money(day.countries[0].id, 1).unwrap();
        assert_eq!(day.gatherer_problem(GathererKind::Woodcutter), None);
        day.build_gatherer_claiming(GathererKind::Woodcutter, 0.0, 0.0, 1).unwrap();
        assert_eq!(day.countries[0].money, 0);
        // With no country there is nobody to own it.
        let mut empty = DaySave::new(Date::FIRST, "Assets/map.nworld");
        assert!(empty.gatherer_problem(GathererKind::Woodcutter).unwrap().contains("no country"));
        assert!(empty.build_gatherer(GathererKind::Woodcutter, 0.0, 0.0, 1).is_err());
    }

    #[test]
    fn there_can_be_only_one_in_a_region_and_nothing_is_paid_for_the_second() {
        let mut day = day_with_money(5_000);
        day.build_gatherer_claiming(GathererKind::Woodcutter, -100.0, 0.0, 3).unwrap();
        let before = day.clone();
        assert!(day.gatherer_region_problem(3, GathererKind::Woodcutter).unwrap().contains("already a woodcutter"));
        assert!(day.build_gatherer_claiming(GathererKind::Woodcutter, -50.0, 40.0, 3).unwrap_err().contains("already a woodcutter"));
        assert_eq!(day, before);
        // Another region is fine, once it is hers.
        assert!(day.gatherer_region_problem(4, GathererKind::Woodcutter).unwrap().contains("doesn't own this region"));
        day.claim_region(4).unwrap();
        assert_eq!(day.gatherer_region_problem(4, GathererKind::Woodcutter), None);
        day.build_gatherer_claiming(GathererKind::Woodcutter, 50.0, 0.0, 4).unwrap();
        assert_eq!(day.gatherers.len(), 2);
    }

    #[test]
    fn a_region_that_belongs_to_another_country_is_refused() {
        let mut day = day_with_money(5_000);
        day.found_country("Brevon").unwrap();
        let brevon = day.countries[1].id;
        day.set_region_owner(5, Some(brevon)).unwrap();
        let before = day.clone();
        assert!(day.build_gatherer_claiming(GathererKind::Woodcutter, 0.0, 0.0, 5).unwrap_err().contains("belongs to Brevon"));
        assert_eq!(day, before);
        // One that is the player's own is fine.
        day.set_region_owner(6, Some(day.countries[0].id)).unwrap();
        day.build_gatherer_claiming(GathererKind::Woodcutter, 0.0, 0.0, 6).unwrap();
    }

    #[test]
    fn gatherers_are_saved_with_the_day_carry_forward_and_go_with_a_deleted_country() {
        let mut day = day_with_money(5_000);
        day.found_country("Brevon").unwrap();
        day.build_gatherer_claiming(GathererKind::Woodcutter, -100.0, 20.0, 7).unwrap();
        let json = serde_json::to_value(&day).unwrap();
        assert_eq!(json["gatherers"][0]["region"], 7);
        assert_eq!(serde_json::from_value::<DaySave>(json).unwrap(), day);
        assert_eq!(day.following(Date { year: 1, month: 1, day: 2 }).gatherers, day.gatherers);
        // A day saved before there were gatherers has none.
        let mut old = serde_json::to_value(&day).unwrap();
        old.as_object_mut().unwrap().remove("gatherers");
        assert!(serde_json::from_value::<DaySave>(old).unwrap().gatherers.is_empty());
        // The other country, which owns nothing, can go; the one that owns the gatherer can't.
        day.delete_country(day.countries[1].id, None).unwrap();
        assert_eq!(day.gatherers.len(), 1);
        let before = day.clone();
        assert_eq!(day.delete_country(day.countries[0].id, None).unwrap_err(), "Aldoria owns 1 woodcutter and 1 region, so it can't be deleted.");
        assert_eq!(day, before);
    }
}
