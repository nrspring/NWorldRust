//! What the units and the buildings look like: each is a small model, like a painted miniature on its base, built into a
//! [`Model`] in its own space, where 1 is the radius of a token and the ground is at 0.
//!
//! They are made to be told apart at a glance, from the side and from above, close up and from far off:
//!
//! - **What moves stands on a round base, and what is built on a square plot.** A unit's base is a metal rim round a disc of its
//!   country's color; a building's plot is a dark slate slab with a band of its country's color round the ground it stands on.
//! - **Each has a shape of its own**, which shows from the side, and **a color of its own**, which shows from above: the green of
//!   the woodcutter's pines, the red roof and the water wheel of the sawmill, the pale cut stone of the stonecutter and the
//!   stepped face of the quarry, the headframe and the rust red ore of the iron mine, the chimney and the green ore and bright
//!   ingots of the copper mine, the yellow crater of the sulphur pit, the tall derrick, the red pump and the black tank of the
//!   oil well, the blue slip and the ribs of a ship being built in a shipyard, the walls and corner towers of a barracks, and the
//!   palisade, square towers or bastions of a small, medium or large fort.
//! - **Militia and soldiers** are figures, more of them the more members there are: militia in brown, with straw hats, spears
//!   and a pennant, on a gold rim; soldiers in their country's color, with steel helmets, pikes and shields and a square banner,
//!   on a steel rim.
//! - **Archers** are militia that have been trained at an archery range: figures in green hoods and tunics with a bow in one hand and
//!   a quiver of feathered arrows on the back, on a green rim, with a pennant. An archery range is a thatched shed with a row of
//!   round targets on hay bales down the range, behind a low rail to shoot from.
//! - **Cavalry** are militia that have been trained at a stable: up to three horsemen, each a rider in steel with a lance on a bay,
//!   dark or grey horse with a saddle cloth of the country's color, on a copper rim, with a pennant. A stable is a long barn with a
//!   red roof beside a fenced paddock with a horse in it.
//! - **People** are one figure on a round silver rim: in the country's colors, with a gold cap and a staff.
//! - **Warships** are hulls with sails, in a gold ring on the water: a small one has one mast and a striped sail, and a large one is
//!   longer and has three masts and a row of gun ports.
//!
//! Their flags wave, the wheels and pumps of a gatherer that is working turn, its fires smoke, and ships rock on the water. This
//! is the game's own drawing, and not part of drawing the map.

use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI, SQRT_2, TAU};

use macroquad::prelude::*;

use crate::fort::FortSize;
use crate::model::{shade, Finish, Model};

/// What a thing looks like.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Look {
    /// A militia of `figures` figures (1 to 5), which are soldiers if it is `trained`.
    Militia { figures: usize, trained: bool },
    /// A militia of `figures` figures (1 to 5) that has been trained as archers.
    Archers { figures: usize },
    /// A militia of `figures` horsemen (1 to 3) that has been trained as cavalry.
    Cavalry { figures: usize },
    /// A person of a country: one figure with a staff.
    Person,
    /// A warship, small or large.
    Ship { large: bool },
    Woodcutter,
    Sawmill,
    Stonecutter,
    Quarry,
    IronMine,
    CopperMine,
    SulphurPit,
    OilWell,
    /// A shipyard, small or large.
    Shipyard { large: bool },
    Barracks,
    ArcheryRange,
    Stable,
    /// A fort of one of its three sizes.
    Fort { size: FortSize },
}

impl Look {
    /// How far its base reaches out from its middle, in the radius of a token: the radius of a unit's base, or half the side
    /// of a building's plot.
    pub fn reach(self) -> f32 {
        match self {
            Look::Militia { .. } | Look::Archers { .. } | Look::Cavalry { .. } | Look::Person => UNIT,
            Look::Ship { large } => if large { 1.3 } else { 1.0 },
            Look::Woodcutter | Look::Stonecutter => 1.0,
            Look::Sawmill | Look::Quarry | Look::IronMine | Look::CopperMine | Look::SulphurPit | Look::OilWell => 1.25,
            Look::Shipyard { large } => if large { 1.6 } else { 1.25 },
            Look::Barracks => 1.35,
            Look::ArcheryRange => 1.3,
            Look::Stable => 1.4,
            Look::Fort { size } => match size {
                FortSize::Small => 1.2,
                FortSize::Medium => 1.45,
                FortSize::Large => 1.7,
            },
        }
    }

    /// How high it stands, in the radius of a token.
    pub fn height(self) -> f32 {
        match self {
            Look::Militia { trained, .. } => UNIT * if trained { 1.75 } else { 1.6 },
            Look::Archers { .. } => UNIT * 1.6,
            Look::Person => UNIT * 1.65,
            Look::Cavalry { .. } => UNIT * 1.7,
            Look::Ship { large } => if large { 1.95 } else { 1.45 },
            Look::Woodcutter => 1.22,
            Look::Sawmill => 1.25,
            Look::Stonecutter => 1.25,
            Look::Quarry => 1.65,
            Look::IronMine => 1.95,
            Look::CopperMine => 1.95,
            Look::SulphurPit => 0.7,
            Look::OilWell => 2.2,
            Look::Shipyard { large } => if large { 1.85 } else { 1.75 },
            Look::Barracks => 1.85,
            Look::ArcheryRange => 1.62,
            Look::Stable => 1.7,
            Look::Fort { size } => match size {
                FortSize::Small => 1.65,
                FortSize::Medium => 1.9,
                FortSize::Large => 2.25,
            },
        }
    }
}

/// How big a militia is drawn, against the radius of a token: a little bigger, so that it is as easy to see as a building.
const UNIT: f32 = 1.15;

/// The time, in seconds, that things that move go by, and whether a gatherer is working (its wheels turn and its fires smoke
/// only then).
#[derive(Clone, Copy, Debug)]
pub struct Moment {
    pub time: f32,
    pub busy: bool,
}

/// Builds the model of `look` into `m`, in the color of `country`, as it is at `moment`.
pub fn build(m: &mut Model, look: Look, country: Color, moment: Moment) {
    match look {
        Look::Militia { figures, trained } => m.scaled(Vec3::ZERO, UNIT, |m| militia(m, figures, trained, country, moment.time)),
        Look::Archers { figures } => m.scaled(Vec3::ZERO, UNIT, |m| archers(m, figures, country, moment.time)),
        Look::Cavalry { figures } => m.scaled(Vec3::ZERO, UNIT, |m| cavalry(m, figures, country, moment.time)),
        Look::Person => m.scaled(Vec3::ZERO, UNIT, |m| a_person(m, country, moment.time)),
        Look::Ship { large } => ship(m, large, country, moment.time),
        Look::Woodcutter => woodcutter(m, country),
        Look::Sawmill => sawmill(m, country, moment),
        Look::Stonecutter => stonecutter(m, country),
        Look::Quarry => quarry(m, country),
        Look::IronMine => iron_mine(m, country, moment),
        Look::CopperMine => copper_mine(m, country, moment),
        Look::SulphurPit => sulphur_pit(m, country, moment),
        Look::OilWell => oil_well(m, country, moment),
        Look::Shipyard { large } => shipyard(m, large, country),
        Look::Barracks => barracks(m, country, moment.time),
        Look::ArcheryRange => archery_range(m, country, moment.time),
        Look::Stable => stable(m, country, moment.time),
        Look::Fort { size } => fort(m, size, country, moment.time),
    }
}

/// Builds a simple model of `look` into `m`, in the color of `country`, for when it is far off and too small on the screen for
/// the detail to show: the same base and the same dominant colors and outline as [`build`], from a handful of shapes, and still.
pub fn build_simple(m: &mut Model, look: Look, country: Color) {
    match look {
        Look::Militia { figures, trained } => m.scaled(Vec3::ZERO, UNIT, |m| {
            let floor = unit_base(m, if trained { STEEL } else { GOLD }, country);
            for (k, (x, z)) in formation(figures).iter().enumerate() {
                let (tunic, head) = if trained { (country, shade(country, 1.25)) } else { (TUNICS[k % TUNICS.len()], STRAW) };
                let foot = v(x * 1.1, floor, z * 1.1);
                m.column(foot, 0.2, 0.12, 0.85, 6, tunic, Finish::Matte);
                m.ball(foot + Vec3::Y * 1.0, v(0.16, 0.16, 0.16), 6, head, Finish::Matte);
            }
            let top = if trained { 1.5 } else { 1.38 };
            m.rod(v(-0.62, floor, -0.5), v(-0.62, floor + top, -0.5), 0.03, WOOD_DARK);
            m.cuboid(v(-0.62, floor + top - 0.14, -0.5 + 0.2), v(0.02, 0.26, 0.4), country, Finish::Matte);
        }),
        Look::Archers { figures } => m.scaled(Vec3::ZERO, UNIT, |m| {
            let floor = unit_base(m, PINE, country);
            for (x, z) in formation(figures) {
                let foot = v(x * 1.1, floor, z * 1.1);
                m.column(foot, 0.2, 0.12, 0.85, 6, ARCHER_GREEN, Finish::Matte);
                m.ball(foot + Vec3::Y * 1.0, v(0.16, 0.16, 0.16), 6, shade(PINE, 1.15), Finish::Matte);
                m.rod(foot + v(0.3, 0.2, -0.15), foot + v(0.3, 1.0, -0.15), 0.03, WOOD_DARK);
            }
            m.rod(v(-0.62, floor, -0.5), v(-0.62, floor + 1.38, -0.5), 0.03, WOOD_DARK);
            m.cuboid(v(-0.62, floor + 1.24, -0.5 + 0.2), v(0.02, 0.26, 0.4), country, Finish::Matte);
        }),
        Look::Person => m.scaled(Vec3::ZERO, UNIT, |m| {
            let floor = unit_base(m, SILVER, country);
            m.column(v(0.0, floor, 0.0), 0.28, 0.14, 1.0, 6, country, Finish::Matte);
            m.ball(v(0.0, floor + 1.12, 0.0), v(0.17, 0.17, 0.17), 6, GOLD, Finish::Metal);
            m.rod(v(0.3, floor, 0.2), v(0.3, floor + 1.3, 0.2), 0.04, WOOD_DARK);
        }),
        Look::Cavalry { figures } => m.scaled(Vec3::ZERO, UNIT, |m| {
            let floor = unit_base(m, CAVALRY_RIM, country);
            for (k, (x, z)) in formation(figures).iter().enumerate() {
                let foot = v(x * 1.1, floor, z * 1.1);
                m.ball(foot + v(0.0, 0.42, 0.0), v(0.5, 0.2, 0.18), 6, HORSE_COATS[k % HORSE_COATS.len()], Finish::Matte);
                m.column(foot + v(0.0, 0.5, 0.0), 0.16, 0.1, 0.55, 6, country, Finish::Matte);
                m.ball(foot + v(0.0, 1.14, 0.0), v(0.14, 0.14, 0.14), 6, shade(country, 1.25), Finish::Matte);
            }
            m.rod(v(-0.62, floor, -0.5), v(-0.62, floor + 1.5, -0.5), 0.03, WOOD_DARK);
            m.cuboid(v(-0.62, floor + 1.36, -0.5 + 0.2), v(0.02, 0.26, 0.4), country, Finish::Matte);
        }),
        Look::Ship { large } => {
            let ring = if large { 1.3 } else { 1.0 };
            m.ring(Vec3::ZERO, ring * 0.85, ring, 0.03, 24, GOLD, Finish::Metal);
            m.set_floor(-1.0);
            if large {
                m.ball(v(0.0, 0.08, 0.0), v(1.15, 0.2, 0.36), 8, HULL_TAR, Finish::Matte);
                for (x, height) in [(-0.62, 1.3), (0.02, 1.75), (0.6, 1.4)] {
                    m.rod(v(x, 0.2, 0.0), v(x, height + 0.15, 0.0), 0.035, WOOD_DARK);
                    m.cuboid(v(x + 0.04, 0.2 + height * 0.55, 0.0), v(0.05, height * 0.8, 0.8), CLOTH, Finish::Matte);
                }
                m.cuboid(v(0.06, 0.55, 0.0), v(0.055, 0.3, 0.8), country, Finish::Matte);
            } else {
                m.ball(v(0.0, 0.07, 0.0), v(0.85, 0.16, 0.28), 8, HULL_WOOD, Finish::Matte);
                m.rod(v(0.05, 0.15, 0.0), v(0.05, 1.4, 0.0), 0.03, WOOD_DARK);
                m.cuboid(v(0.09, 0.8, 0.0), v(0.05, 1.0, 0.8), country, Finish::Matte);
            }
        }
        Look::Woodcutter => {
            let f = plot(m, Look::Woodcutter.reach(), GRASS, country);
            m.column(v(-0.4, f, -0.3), 0.36, 0.0, 1.0, 6, PINE, Finish::Matte);
            m.column(v(-0.4, f, 0.4), 0.3, 0.0, 0.8, 6, shade(PINE, 1.2), Finish::Matte);
            m.block(v(0.4, f, 0.1), v(0.45, 0.3, 0.45), THATCH);
        }
        Look::Sawmill => {
            let f = plot(m, Look::Sawmill.reach(), DIRT, country);
            house(m, v(0.0, f, 0.0), v(0.95, 0.45, 0.65), 0.0, WOOD, ROOF_RED, 0.3);
        }
        Look::Stonecutter => {
            let f = plot(m, Look::Stonecutter.reach(), DIRT, country);
            m.block(v(0.0, f, 0.0), v(0.8, 0.5, 0.6), STONE_LIGHT);
            m.block(v(-0.3, f, 0.5), v(0.4, 0.25, 0.3), STONE);
        }
        Look::Quarry => {
            let f = plot(m, Look::Quarry.reach(), GRAVEL, country);
            for (k, size) in [1.4, 1.0, 0.6].into_iter().enumerate() {
                m.block(v(0.0, f + 0.3 * k as f32, 0.0), v(size, 0.3, size), if k % 2 == 0 { STONE_DARK } else { STONE_LIGHT });
            }
        }
        Look::IronMine => {
            let f = plot(m, Look::IronMine.reach(), DIRT, country);
            m.block(v(0.0, f, 0.0), v(0.8, 0.35, 0.7), RUST);
            m.column(v(0.1, f, 0.0), 0.18, 0.06, 1.7, 4, IRON, Finish::Metal);
        }
        Look::CopperMine => {
            let f = plot(m, Look::CopperMine.reach(), RED_EARTH, country);
            m.block(v(0.0, f, 0.0), v(0.8, 0.4, 0.7), MALACHITE);
            m.block(v(0.5, f, -0.4), v(0.3, 0.2, 0.3), COPPER);
            m.column(v(-0.3, f, 0.3), 0.14, 0.1, 1.6, 6, BRICK, Finish::Matte);
        }
        Look::SulphurPit => {
            let f = plot(m, Look::SulphurPit.reach(), ASH, country);
            m.column(v(0.0, f, 0.0), 0.7, 0.55, 0.12, 12, SULPHUR, Finish::Matte);
            m.column(v(0.0, f + 0.12, 0.0), 0.5, 0.2, 0.35, 8, shade(SULPHUR, 0.85), Finish::Matte);
        }
        Look::OilWell => {
            let f = plot(m, Look::OilWell.reach(), OILY, country);
            m.column(v(-0.2, f, 0.0), 0.35, 0.05, 2.0, 4, WOOD_DARK, Finish::Matte);
            m.column(v(0.45, f, 0.35), 0.35, 0.35, 0.45, 8, TANK, Finish::Metal);
            m.block(v(0.3, f, -0.45), v(0.55, 0.3, 0.2), PUMP_RED);
        }
        Look::Shipyard { large } => {
            let half = Look::Shipyard { large }.reach();
            let f = plot(m, half, SAND, country);
            let inner = half * 0.88;
            m.cuboid(v(inner * 0.71, f + 0.004, 0.0), v(inner * 0.58, 0.012, inner * 2.0), WATER, Finish::Gloss);
            let length = if large { 2.0 } else { 1.5 };
            m.ball(v(-inner * 0.1, f + 0.3, 0.0), v(length * 0.5, 0.2, 0.3), 8, HULL_WOOD, Finish::Matte);
            m.column(v(-inner * 0.4, f, inner * 0.7), 0.1, 0.05, 1.5, 4, WOOD_DARK, Finish::Matte);
        }
        Look::Barracks => {
            let half = Look::Barracks.reach();
            let f = plot(m, half, GRAVEL, country);
            let w = half * 0.76;
            m.block(v(0.0, f, 0.0), v(w * 2.0, 0.4, w * 2.0), STONE);
            for (x, z) in [(-w, -w), (w, -w), (-w, w), (w, w)] {
                m.column(v(x, f, z), 0.17, 0.17, 0.75, 6, STONE_LIGHT, Finish::Matte);
                m.column(v(x, f + 0.75, z), 0.22, 0.0, 0.3, 6, country, Finish::Gloss);
            }
        }
        Look::Stable => {
            let half = Look::Stable.reach();
            let f = plot(m, half, DIRT, country);
            m.block(v(-0.7, f, 0.0), v(0.55, 0.4, 1.4), WOOD);
            m.block(v(-0.7, f + 0.4, 0.0), v(0.7, 0.14, 1.5), ROOF_RED);
            m.ball(v(0.45, f + 0.3, 0.0), v(0.3, 0.14, 0.12), 6, HORSE_COATS[0], Finish::Matte);
            m.rod(v(0.45, f + 0.3, 0.0), v(0.8, f + 0.6, 0.0), 0.05, HORSE_COATS[0]);
            m.rod(v(-0.2, f, 0.0), v(-0.2, f + 1.1, 0.0), 0.03, WOOD_DARK);
        }
        Look::Fort { size } => {
            let half = look.reach();
            let f = plot(m, half, if size == FortSize::Small { DIRT } else { GRAVEL }, country);
            let w = half * 0.74;
            let (wall_color, tall) = if size == FortSize::Small { (WOOD, 0.34) } else { (STONE, 0.4) };
            m.block(v(0.0, f, 0.0), v(w * 2.0, tall, w * 2.0), wall_color);
            // The keep or watchtower in the middle, and a pole up to the top with the country's pennant.
            let (keep, keep_height) = match size {
                FortSize::Small => (0.36, 0.9),
                FortSize::Medium => (0.8, 1.0),
                FortSize::Large => (0.95, 1.35),
            };
            m.block(v(0.0, f, 0.0), v(keep, keep_height, keep), if size == FortSize::Small { WOOD_DARK } else { STONE_LIGHT });
            let top = look.height() - 0.05;
            m.rod(v(0.0, f + keep_height, 0.0), v(0.0, top, 0.0), 0.03, WOOD_DARK);
            m.cuboid(v(0.18, top - 0.14, 0.0), v(0.36, 0.24, 0.02), country, Finish::Matte);
            if size != FortSize::Small {
                for (x, z) in [(-w, -w), (w, -w), (-w, w), (w, w)] {
                    m.block(v(x, f, z), v(0.34, 0.66, 0.34), STONE_DARK);
                }
            }
        }
        Look::ArcheryRange => {
            let half = Look::ArcheryRange.reach();
            let f = plot(m, half, GRASS, country);
            m.block(v(-half * 0.55, f, 0.0), v(0.5, 0.34, 1.1), WOOD);
            m.block(v(-half * 0.55, f + 0.34, 0.0), v(0.62, 0.12, 1.24), country);
            for z in [-0.55, 0.0, 0.55] {
                m.rod(v(0.8, f, z), v(0.8, f + 0.3, z), 0.03, WOOD_DARK);
                m.ball(v(0.78, f + 0.42, z), v(0.03, 0.2, 0.2), 8, ROOF_RED, Finish::Matte);
            }
            m.rod(v(-0.1, f + 0.9, 0.0), v(-0.1, f + 1.4, 0.0), 0.03, WOOD_DARK);
        }
    }
}

const fn c(r: f32, g: f32, b: f32) -> Color {
    Color::new(r, g, b, 1.0)
}

fn v(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, y, z)
}

/// The gold of the rim of a militia's base and a ship's ring, and the steel of soldiers'.
pub const GOLD: Color = c(0.90, 0.70, 0.22);
pub const STEEL: Color = c(0.62, 0.65, 0.70);
/// The dark slate of a building's plot.
pub const SLATE: Color = c(0.20, 0.22, 0.27);
/// The pale of cloth: sails, and the stripes of flags.
pub const CLOTH: Color = c(0.92, 0.89, 0.80);

const SKIN: Color = c(0.86, 0.64, 0.48);
const BOOT: Color = c(0.18, 0.13, 0.10);
const LEATHER: Color = c(0.40, 0.26, 0.14);
const STRAW: Color = c(0.86, 0.74, 0.40);
const TUNICS: [Color; 3] = [c(0.50, 0.34, 0.20), c(0.62, 0.50, 0.28), c(0.40, 0.42, 0.30)];
const BREECHES: Color = c(0.30, 0.24, 0.18);
const ARCHER_GREEN: Color = c(0.26, 0.40, 0.22);
/// The copper of the rim of cavalry's base, and the coats a horse can have: bay, dark brown and grey.
/// The silver of the rim of a person's base.
const SILVER: Color = c(0.84, 0.86, 0.90);
const CAVALRY_RIM: Color = c(0.72, 0.40, 0.24);
const HORSE_COATS: [Color; 3] = [c(0.48, 0.29, 0.16), c(0.20, 0.16, 0.14), c(0.74, 0.72, 0.66)];

const BARK: Color = c(0.36, 0.24, 0.15);
const WOOD: Color = c(0.52, 0.36, 0.21);
const WOOD_DARK: Color = c(0.30, 0.20, 0.13);
const PLANK: Color = c(0.82, 0.66, 0.42);
const PINE: Color = c(0.12, 0.40, 0.17);
const THATCH: Color = c(0.74, 0.60, 0.32);
const ROOF_RED: Color = c(0.66, 0.22, 0.15);
const ROOF_SLATE: Color = c(0.30, 0.34, 0.42);
const PATINA: Color = c(0.30, 0.60, 0.50);

const STONE: Color = c(0.60, 0.59, 0.56);
const STONE_LIGHT: Color = c(0.82, 0.80, 0.74);
const STONE_DARK: Color = c(0.38, 0.38, 0.40);
const ROCK: Color = c(0.36, 0.33, 0.31);
const BRICK: Color = c(0.56, 0.26, 0.18);
const COAL: Color = c(0.07, 0.07, 0.08);

const GRASS: Color = c(0.34, 0.50, 0.22);
const DIRT: Color = c(0.48, 0.37, 0.24);
const RED_EARTH: Color = c(0.56, 0.38, 0.27);
const GRAVEL: Color = c(0.62, 0.59, 0.52);
const SAND: Color = c(0.84, 0.75, 0.54);
const ASH: Color = c(0.68, 0.66, 0.52);
const OILY: Color = c(0.30, 0.26, 0.22);
const WATER: Color = c(0.16, 0.42, 0.68);

const IRON: Color = c(0.40, 0.42, 0.47);
const RUST: Color = c(0.62, 0.24, 0.13);
const COPPER: Color = c(0.92, 0.50, 0.22);
const MALACHITE: Color = c(0.22, 0.58, 0.44);
const SULPHUR: Color = c(0.96, 0.84, 0.16);
const SMOKE: Color = c(0.46, 0.45, 0.45);
const STEAM: Color = c(0.90, 0.92, 0.90);
const FIRE: Color = c(1.0, 0.55, 0.15);
const BLUE_FLAME: Color = c(0.35, 0.60, 1.0);
const LAMP: Color = c(1.0, 0.82, 0.45);
const OIL: Color = c(0.04, 0.04, 0.05);
const TANK: Color = c(0.22, 0.23, 0.26);
const PUMP_RED: Color = c(0.72, 0.16, 0.10);
const HULL_WOOD: Color = c(0.46, 0.30, 0.17);
const HULL_TAR: Color = c(0.17, 0.14, 0.12);
const HULL_LOW: Color = c(0.24, 0.15, 0.10);

// ---------------------------------------------------------------------------------------------------------------------------
// Bases.

/// The round base of a unit: a rim of `rim` round a disc of the country's color. The figures stand on it.
fn unit_base(m: &mut Model, rim: Color, country: Color) -> f32 {
    m.column(Vec3::ZERO, 1.0, 0.95, 0.1, 40, shade(rim, 0.8), Finish::Metal);
    m.column(Vec3::Y * 0.1, 0.95, 0.9, 0.025, 40, rim, Finish::Metal);
    m.column(Vec3::Y * 0.1, 0.82, 0.82, 0.03, 40, country, Finish::Gloss);
    m.set_floor(0.13);
    0.13
}

/// The square plot of a building, of `half` a side: a dark slate slab with sloping sides, a band of the country's color, and the
/// ground the building stands on. The height of its top.
fn plot(m: &mut Model, half: f32, ground: Color, country: Color) -> f32 {
    let h = 0.14;
    m.frustum(Vec3::ZERO, Vec3::Y * h, half * 1.07 * SQRT_2, half * SQRT_2, 4, FRAC_PI_4, SLATE, Finish::Matte);
    m.cuboid(Vec3::Y * (h + 0.025), v(half * 2.0, 0.05, half * 2.0), country, Finish::Gloss);
    m.block(Vec3::Y * (h + 0.05), v(half * 1.76, 0.02, half * 1.76), ground);
    let top = h + 0.07;
    m.set_floor(top);
    top
}

// ---------------------------------------------------------------------------------------------------------------------------
// Bits that are used more than once.

/// A pine of `s` (1 is about 1 high) standing at `foot`: a trunk and three tiers of branches, lighter towards the top.
fn pine(m: &mut Model, foot: Vec3, s: f32) {
    m.column(foot, 0.06 * s, 0.05 * s, 0.22 * s, 6, BARK, Finish::Matte);
    for (k, (r, h, light)) in [(0.36, 0.46, 0.8), (0.28, 0.4, 1.0), (0.19, 0.36, 1.22)].into_iter().enumerate() {
        m.column(foot + Vec3::Y * (0.16 + 0.24 * k as f32) * s, r * s, 0.0, h * s, 10, shade(PINE, light), Finish::Matte);
    }
}

/// A log lying from `a` to `b`, with pale cut ends.
fn log(m: &mut Model, a: Vec3, b: Vec3, r: f32) {
    m.frustum(a, b, r, r, 8, 0.0, BARK, Finish::Matte);
    let dir = (b - a).normalize();
    m.frustum(a - dir * 0.004, a, r * 0.86, r * 0.86, 8, 0.0, PLANK, Finish::Matte);
    m.frustum(b, b + dir * 0.004, r * 0.86, r * 0.86, 8, 0.0, PLANK, Finish::Matte);
}

/// A stack of `layers` of planks on `foot`, of `length` along x, each layer a little turned from the last.
fn planks(m: &mut Model, foot: Vec3, length: f32, layers: usize) {
    for k in 0..layers {
        let turn = if k % 2 == 0 { 0.0 } else { 0.06 };
        let tone = if k % 2 == 0 { 1.0 } else { 0.88 };
        m.block_turned(foot + Vec3::Y * 0.05 * k as f32, v(length, 0.05, 0.3), turn, shade(PLANK, tone));
    }
}

/// A house of `size` on `foot`, with walls of `wall` and a pitched roof of `roof` along x turned by `yaw`, and a door at the front.
fn house(m: &mut Model, foot: Vec3, size: Vec3, yaw: f32, wall: Color, roof: Color, pitch: f32) {
    m.block_turned(foot, size, yaw, wall);
    let turn = Mat3::from_rotation_y(yaw);
    m.roof(foot + Vec3::Y * size.y, size.x + 0.12, size.z + 0.14, pitch, yaw, roof, wall);
    m.oriented_box(foot + turn * v(0.0, 0.1, size.z * 0.5 + 0.005), turn, v(0.1, 0.2, 0.01), WOOD_DARK, Finish::Matte);
}

/// Puffs of `color` going up from `from` and drifting off, of `size` at first and bigger as they go, `count` of them at a time.
fn puffs(m: &mut Model, from: Vec3, time: f32, count: usize, rise: f32, size: f32, color: Color) {
    for k in 0..count {
        let p = (time * 0.32 + k as f32 / count as f32).fract();
        let at = from + v(p * p * 0.45, p * rise, p * 0.15);
        let r = size * (0.5 + p * 1.5);
        m.ball(at, v(r * 1.2, r * 0.85, r * 1.2), 10, shade(color, 1.0 + p * 0.2), Finish::Matte);
    }
}

/// A flag on a pole standing on `foot`, `height` high, waving at `time`. A square banner has a pale cross on it, and a
/// pennant narrows to a point.
fn flag(m: &mut Model, foot: Vec3, height: f32, country: Color, time: f32, banner: bool, size: f32) {
    m.frustum(foot, foot + Vec3::Y * height, 0.02, 0.016, 6, 0.0, WOOD_DARK, Finish::Matte);
    m.ball(foot + Vec3::Y * (height + 0.03), Vec3::splat(0.036), 8, GOLD, Finish::Metal);
    let top = foot + Vec3::Y * (height - 0.03);
    let (length, tall) = if banner { (0.52 * size, 0.34 * size) } else { (0.6 * size, 0.3 * size) };
    m.sheet(
        10,
        5,
        |u, w| {
            let wave = (u * 5.5 - time * 5.0).sin() * 0.05 * u * size;
            let drop = if banner { w * tall } else { tall * 0.5 + (w - 0.5) * tall * (1.0 - u * 0.92) };
            top + v(-u * length, -drop - u * u * 0.04 * size, wave)
        },
        |u, w| if banner && ((w - 0.5).abs() < 0.12 || (u - 0.35).abs() < 0.07) { CLOTH } else { country },
        Finish::Matte,
    );
}

// ---------------------------------------------------------------------------------------------------------------------------
// Militia and soldiers.

#[derive(Clone, Copy)]
enum Hat {
    Straw,
    Cap(Color),
    Helmet(Color),
}

#[derive(Clone, Copy)]
struct Kit {
    tunic: Color,
    /// Whether the body is in steel, with the tunic only below it and on the arms.
    armor: bool,
    legs: Color,
    hat: Hat,
    /// The height of the top of the spear or pike, and the shield on the other arm, if there is one.
    reach: f32,
    shield: Option<Color>,
    /// What it carries.
    arms: Arms,
}

/// What a figure carries.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Arms {
    /// A spear or a pike, in the right hand.
    Spear,
    /// A bow in the left hand, and a quiver on the back.
    Bow,
    /// A walking staff with a gold knob, in the right hand.
    Staff,
}

/// A figure standing at `foot`, facing `face` (0 is the front of the model, +x), dressed and armed as `kit` says.
fn person(m: &mut Model, foot: Vec3, face: f32, kit: Kit) {
    let turn = Mat3::from_rotation_y(face);
    let at = |x: f32, y: f32, z: f32| foot + turn * v(x, y, z);
    for side in [-1.0, 1.0] {
        m.oriented_box(at(0.0, 0.12, 0.045 * side), turn, v(0.07, 0.2, 0.065), kit.legs, Finish::Matte);
        m.oriented_box(at(0.012, 0.025, 0.045 * side), turn, v(0.1, 0.05, 0.075), BOOT, Finish::Matte);
    }
    let (body, finish) = if kit.armor { (STEEL, Finish::Metal) } else { (kit.tunic, Finish::Matte) };
    m.frustum(at(0.0, 0.17, 0.0), at(0.0, 0.29, 0.0), 0.13, 0.118, 7, 0.0, kit.tunic, Finish::Matte);
    m.frustum(at(0.0, 0.27, 0.0), at(0.0, 0.47, 0.0), 0.118, 0.1, 7, 0.0, body, finish);
    m.frustum(at(0.0, 0.27, 0.0), at(0.0, 0.305, 0.0), 0.124, 0.122, 7, 0.0, LEATHER, Finish::Matte);
    m.ball(at(0.0, 0.46, 0.0), v(0.12, 0.05, 0.13), 7, body, finish);
    let (right, left) = (at(0.09, 0.32, 0.14), at(0.11, 0.31, -0.11));
    m.frustum(at(0.0, 0.44, 0.115), right, 0.034, 0.028, 6, 0.0, kit.tunic, Finish::Matte);
    m.frustum(at(0.0, 0.44, -0.115), left, 0.034, 0.028, 6, 0.0, kit.tunic, Finish::Matte);
    m.ball(right, Vec3::splat(0.03), 4, SKIN, Finish::Matte);
    m.ball(left, Vec3::splat(0.03), 4, SKIN, Finish::Matte);
    m.ball(at(0.0, 0.56, 0.0), v(0.072, 0.08, 0.072), 10, SKIN, Finish::Matte);
    match kit.hat {
        Hat::Straw => {
            m.column(at(0.0, 0.59, 0.0), 0.155, 0.145, 0.016, 12, STRAW, Finish::Matte);
            m.dome(at(0.0, 0.6, 0.0), v(0.072, 0.065, 0.072), 10, shade(STRAW, 1.05), Finish::Matte);
        }
        Hat::Cap(color) => m.dome(at(0.0, 0.575, 0.0), v(0.083, 0.07, 0.083), 10, color, Finish::Matte),
        Hat::Helmet(crest) => {
            m.dome(at(0.0, 0.57, 0.0), v(0.088, 0.088, 0.088), 12, STEEL, Finish::Metal);
            m.column(at(0.0, 0.565, 0.0), 0.115, 0.11, 0.014, 12, STEEL, Finish::Metal);
            m.oriented_box(at(-0.01, 0.665, 0.0), turn, v(0.14, 0.05, 0.026), crest, Finish::Matte);
        }
    }
    if kit.arms == Arms::Bow {
        // A bow held out in front by the left hand, bending forward in the middle, with its string, and a quiver of arrows on the back.
        let limb = |t: f32| at(0.15 + 0.07 * (1.0 - t * t), 0.34 + 0.3 * t, -0.11);
        for k in 0..4 {
            let (a, b) = (limb(k as f32 / 2.0 - 1.0), limb((k + 1) as f32 / 2.0 - 1.0));
            m.rod(a, b, 0.014, WOOD_DARK);
        }
        m.rod(limb(-1.0), limb(1.0), 0.005, CLOTH);
        m.frustum(at(-0.09, 0.18, 0.06), at(-0.14, 0.5, 0.03), 0.04, 0.046, 6, 0.0, LEATHER, Finish::Matte);
        m.ball(at(-0.14, 0.53, 0.03), Vec3::splat(0.032), 5, CLOTH, Finish::Matte);
    } else if kit.arms == Arms::Staff {
        let (foot, top) = (at(0.09, 0.0, 0.14), at(0.09, kit.reach, 0.14));
        m.frustum(foot, top, 0.016, 0.013, 5, 0.0, WOOD, Finish::Matte);
        m.ball(top + Vec3::Y * 0.02, Vec3::splat(0.036), 8, GOLD, Finish::Metal);
    } else {
        // The spear or pike in the right hand, with a steel point.
        let shaft = at(0.09, 0.02, 0.14);
        let tip = at(0.09, kit.reach, 0.14);
        m.frustum(shaft, tip, 0.013, 0.011, 5, 0.0, WOOD, Finish::Matte);
        m.frustum(tip, tip + Vec3::Y * 0.11, 0.03, 0.0, 6, 0.0, STEEL, Finish::Metal);
    }
    if let Some(color) = kit.shield {
        m.oriented_box(at(0.145, 0.33, -0.11), turn, v(0.02, 0.3, 0.23), STEEL, Finish::Metal);
        m.oriented_box(at(0.158, 0.33, -0.11), turn, v(0.012, 0.26, 0.19), color, Finish::Gloss);
        m.ball(at(0.166, 0.34, -0.11), Vec3::splat(0.032), 8, GOLD, Finish::Metal);
    }
}

/// How big a figure is, against the base it stands on.
const FIGURE: f32 = 1.35;

/// Where the figures of a militia of `count` stand on its base.
fn formation(count: usize) -> &'static [(f32, f32)] {
    match count {
        0 | 1 => &[(0.1, 0.05)],
        2 => &[(0.2, -0.3), (0.2, 0.3)],
        3 => &[(0.34, 0.0), (-0.08, -0.36), (-0.08, 0.36)],
        4 => &[(0.36, -0.28), (0.36, 0.28), (-0.12, -0.28), (-0.12, 0.28)],
        _ => &[(0.46, 0.0), (0.14, -0.4), (0.14, 0.4), (-0.24, -0.2), (-0.24, 0.24)],
    }
}

fn militia(m: &mut Model, figures: usize, trained: bool, country: Color, time: f32) {
    let floor = unit_base(m, if trained { STEEL } else { GOLD }, country);
    for (k, (x, z)) in formation(figures).iter().enumerate() {
        let kit = if trained {
            Kit { tunic: country, armor: true, legs: c(0.24, 0.24, 0.26), hat: Hat::Helmet(shade(country, 1.25)), reach: 1.04, shield: Some(country), arms: Arms::Spear }
        } else {
            let hat = if k % 3 == 1 { Hat::Cap(shade(country, 0.85)) } else { Hat::Straw };
            Kit { tunic: TUNICS[k % TUNICS.len()], armor: false, legs: BREECHES, hat, reach: 0.92, shield: None, arms: Arms::Spear }
        };
        // Each a little turned from the next, so they don't stand like posts.
        let face = ((k * 7) % 5) as f32 * 0.08 - 0.16;
        m.scaled(v(x * 1.1, floor, z * 1.1), FIGURE, |m| person(m, Vec3::ZERO, face, kit));
    }
    flag(m, v(-0.62, floor, -0.5), if trained { 1.5 } else { 1.38 }, country, time, trained, 1.0);
}

/// A person of a country, standing on a silver rim: in the country's colors, with a gold cap and a staff.
fn a_person(m: &mut Model, country: Color, time: f32) {
    let floor = unit_base(m, SILVER, country);
    let kit = Kit { tunic: country, armor: false, legs: shade(country, 0.6), hat: Hat::Cap(GOLD), reach: 0.95, shield: None, arms: Arms::Staff };
    m.scaled(v(0.05, floor, 0.0), 1.5, |m| person(m, Vec3::ZERO, 0.0, kit));
    // And their own standard beside them.
    flag(m, v(-0.6, floor, -0.45), 1.3, country, time, true, 0.9);
}

/// A militia trained as archers, of `figures` figures: in green, with a hood, a bow and a quiver, on a green rim.
fn archers(m: &mut Model, figures: usize, country: Color, time: f32) {
    let floor = unit_base(m, PINE, country);
    for k in 0..formation(figures).len() {
        let (x, z) = formation(figures)[k];
        let kit = Kit { tunic: ARCHER_GREEN, armor: false, legs: BREECHES, hat: Hat::Cap(shade(PINE, 1.15)), reach: 0.0, shield: None, arms: Arms::Bow };
        let face = ((k * 7) % 5) as f32 * 0.08 - 0.16;
        m.scaled(v(x * 1.1, floor, z * 1.1), FIGURE, |m| person(m, Vec3::ZERO, face, kit));
    }
    flag(m, v(-0.62, floor, -0.5), 1.38, country, time, false, 1.0);
}

/// A horse standing at `foot`, facing `face` (0 is +x), of coat `coat`, with a saddle cloth of `cloth`, as big as a horse is against a
/// man on foot (about 0.5 high at the back and 0.6 long).
fn horse(m: &mut Model, foot: Vec3, face: f32, coat: Color, cloth: Color) {
    let turn = Mat3::from_rotation_y(face);
    let at = |x: f32, y: f32, z: f32| foot + turn * v(x, y, z);
    let dark = shade(coat, 0.55);
    // The body and the rump, the chest, and the neck going up to the head, with its snout, ears and mane.
    m.ball(at(0.0, 0.36, 0.0), v(0.3, 0.14, 0.12), 12, coat, Finish::Matte);
    m.ball(at(-0.14, 0.38, 0.0), v(0.17, 0.15, 0.125), 10, coat, Finish::Matte);
    m.ball(at(0.2, 0.39, 0.0), v(0.14, 0.15, 0.11), 10, coat, Finish::Matte);
    m.frustum(at(0.24, 0.42, 0.0), at(0.4, 0.68, 0.0), 0.07, 0.045, 8, 0.0, coat, Finish::Matte);
    m.ball(at(0.46, 0.7, 0.0), v(0.085, 0.052, 0.042), 8, coat, Finish::Matte);
    m.frustum(at(0.49, 0.69, 0.0), at(0.58, 0.62, 0.0), 0.04, 0.028, 6, 0.0, shade(coat, 0.85), Finish::Matte);
    for side in [-1.0, 1.0] {
        m.frustum(at(0.4, 0.74, 0.03 * side), at(0.4, 0.8, 0.035 * side), 0.016, 0.0, 4, 0.0, coat, Finish::Matte);
    }
    m.rod(at(0.4, 0.74, 0.0), at(0.27, 0.54, 0.0), 0.026, dark);
    // The legs, with hooves, and the tail.
    for (x, side) in [(0.21, -1.0), (0.21, 1.0), (-0.24, -1.0), (-0.24, 1.0)] {
        m.frustum(at(x, 0.3, 0.075 * side), at(x, 0.04, 0.075 * side), 0.036, 0.022, 6, 0.0, coat, Finish::Matte);
        m.frustum(at(x, 0.04, 0.075 * side), at(x, 0.0, 0.075 * side), 0.03, 0.034, 6, 0.0, dark, Finish::Matte);
    }
    m.frustum(at(-0.3, 0.42, 0.0), at(-0.42, 0.14, 0.0), 0.03, 0.014, 5, 0.0, dark, Finish::Matte);
    // The saddle cloth, in the color of the country.
    m.oriented_box(at(0.0, 0.5, 0.0), turn, v(0.26, 0.03, 0.27), cloth, Finish::Gloss);
}

/// A militia trained as cavalry, of `figures` horsemen: each a rider in steel with a lance on a horse, on a copper rim, with a pennant.
fn cavalry(m: &mut Model, figures: usize, country: Color, time: f32) {
    let floor = unit_base(m, CAVALRY_RIM, country);
    for k in 0..formation(figures).len() {
        let (x, z) = formation(figures)[k];
        let face = ((k * 7) % 5) as f32 * 0.08 - 0.16;
        let kit = Kit { tunic: country, armor: true, legs: c(0.24, 0.24, 0.26), hat: Hat::Helmet(shade(country, 1.25)), reach: 1.0, shield: None, arms: Arms::Spear };
        m.scaled(v(x * 1.1, floor, z * 1.1), 0.95, |m| {
            horse(m, Vec3::ZERO, face, HORSE_COATS[k % HORSE_COATS.len()], country);
            person(m, Mat3::from_rotation_y(face) * v(-0.02, 0.43, 0.0), face, kit);
        });
    }
    flag(m, v(-0.62, floor, -0.5), 1.5, country, time, false, 1.0);
}

// ---------------------------------------------------------------------------------------------------------------------------
// Ships.

/// The size of a hull: how long and how broad it is, how high its sides stand over the water at the middle, how deep it goes
/// under it, how deep the band of the country's color along its top is, how high it is lifted (0 for one in the water), and how
/// far forward its middle is.
struct Hull {
    length: f32,
    beam: f32,
    height: f32,
    draft: f32,
    wale: f32,
    lift: f32,
    along: f32,
}

impl Hull {
    /// At `t` along it, from the stern (0) to the bow (1): where that is along x, how far out its side is, and the height of the
    /// top of its side and of its keel.
    fn at(&self, t: f32) -> (f32, f32, f32, f32) {
        let x = self.along - self.length * 0.5 + t * self.length;
        let w = if t < 0.42 { 1.0 - ((0.42 - t) / 0.42).powi(2) * 0.24 } else { 1.0 - ((t - 0.42) / 0.58).powf(2.2) };
        let rise = 0.06 * (2.0 * t - 1.0).powi(2) + if t > 0.75 { (t - 0.75) * 0.28 } else { 0.0 };
        let keel = self.draft * (1.0 - smooth(0.68, 1.0, t) * 0.8);
        (x, self.beam * 0.5 * w.max(0.0), self.lift + self.height + rise, self.lift - keel)
    }

    /// The points of its side at `t`, from the top down to the keel, on the side `side` (1 or -1).
    fn section(&self, t: f32, side: f32) -> [Vec3; 5] {
        let (x, w, top, keel) = self.at(t);
        let water = self.lift + 0.01;
        [v(x, top, w * side), v(x, top - self.wale, w * side * 1.01), v(x, water, w * side * 0.96), v(x, water + (keel - water) * 0.62, w * side * 0.62), v(x, keel, 0.0)]
    }
}

fn smooth(from: f32, to: f32, x: f32) -> f32 {
    let t = ((x - from) / (to - from)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

const STATIONS: usize = 14;

/// The hull: its sides, from `t` 0 to `planked` of its length, in bands of the country's color along the top, `upper` down to the
/// water and `lower` under it, and, if it is `decked`, its deck and its stern.
fn hull(m: &mut Model, hull: &Hull, country: Color, upper: Color, lower: Color, planked: f32, decked: bool) {
    let colors = [country, upper, lower, lower];
    let stations = ((STATIONS as f32 * planked).ceil() as usize).max(1);
    for i in 0..stations {
        let (t0, t1) = (i as f32 / STATIONS as f32, ((i + 1) as f32 / STATIONS as f32).min(planked.max(0.0)));
        for side in [-1.0, 1.0] {
            let (a, b) = (hull.section(t0, side), hull.section(t1, side));
            for band in 0..4 {
                m.quad([a[band], b[band], b[band + 1], a[band + 1]], colors[band], if band == 0 { Finish::Gloss } else { Finish::Matte });
            }
        }
        if decked {
            let ((x0, w0, top0, _), (x1, w1, top1, _)) = (hull.at(t0), hull.at(t1));
            let (d0, d1) = (top0 - 0.025, top1 - 0.025);
            m.quad([v(x0, d0, -w0 * 0.97), v(x1, d1, -w1 * 0.97), v(x1, d1, w1 * 0.97), v(x0, d0, w0 * 0.97)], PLANK, Finish::Matte);
        }
    }
    if decked {
        // The stern, flat across.
        let (port, starboard) = (hull.section(0.0, -1.0), hull.section(0.0, 1.0));
        let middle = (port[0] + port[4]) * 0.5 - Vec3::X * 0.002;
        for k in 0..4 {
            m.tri(middle, port[k], port[k + 1], shade(upper, 0.9), Finish::Matte);
            m.tri(middle, starboard[k], starboard[k + 1], shade(upper, 0.9), Finish::Matte);
        }
        m.tri(middle, port[0], starboard[0], shade(upper, 0.9), Finish::Matte);
    }
}

/// A square sail hanging from a yard at `top` on a mast at `x`, `width` across and `drop` down, filled with the wind so that it
/// bellies out forward. `color` gives the color of the cloth at (`u` across, `v` down).
fn square_sail(m: &mut Model, x: f32, top: f32, width: f32, drop: f32, time: f32, color: impl Fn(f32, f32) -> Color) {
    m.frustum(v(x, top + 0.01, -width * 0.56), v(x, top + 0.01, width * 0.56), 0.012, 0.012, 5, 0.0, WOOD_DARK, Finish::Matte);
    let gust = 1.0 + 0.08 * (time * 1.7).sin();
    m.sheet(
        8,
        5,
        |u, w| {
            let across = 1.0 - (2.0 * u - 1.0).powi(2);
            let belly = 0.13 * width * across * (0.35 + 0.65 * (PI * w).sin()) * gust;
            v(x + belly + 0.015, top - w * drop, (u - 0.5) * width * (1.0 - 0.1 * w))
        },
        color,
        Finish::Matte,
    );
}

/// A long, narrow pennant of the country's color flying back from `top`.
fn pennant(m: &mut Model, top: Vec3, length: f32, country: Color, time: f32) {
    m.sheet(
        8,
        1,
        |u, w| top + v(-u * length, -w * 0.07 * (1.0 - u * 0.8) - u * 0.03, (u * 6.0 - time * 6.0).sin() * 0.04 * u),
        |_, _| country,
        Finish::Matte,
    );
}

fn ship(m: &mut Model, large: bool, country: Color, time: f32) {
    let ring = if large { 1.3 } else { 1.0 };
    m.ring(Vec3::ZERO, ring * 0.93, ring, 0.035, 48, GOLD, Finish::Metal);
    m.ring(Vec3::ZERO, ring * 0.85, ring * 0.93, 0.028, 48, country, Finish::Gloss);
    m.set_floor(-1.0);
    if large {
        let h = Hull { length: 2.3, beam: 0.68, height: 0.3, draft: 0.2, wale: 0.13, lift: 0.0, along: 0.0 };
        hull(m, &h, country, HULL_TAR, HULL_LOW, 1.0, true);
        // A row of gun ports along each side, in the band of the country's color.
        for k in 0..6 {
            let t = 0.2 + 0.55 * k as f32 / 5.0;
            let (x, w, top, _) = h.at(t);
            for side in [-1.0, 1.0] {
                m.cuboid(v(x, top - h.wale * 0.5, (w * 1.01 + 0.004) * side), v(0.065, h.wale * 0.5, 0.012), COAL, Finish::Matte);
            }
        }
        // The castle at the stern, with lit windows and a gold rail, and the forecastle.
        let (x0, w0, top0, _) = h.at(0.02);
        m.block(v(x0 + 0.22, top0 - 0.03, 0.0), v(0.44, 0.17, w0 * 1.9), HULL_TAR);
        m.block(v(x0 + 0.22, top0 + 0.14, 0.0), v(0.44, 0.012, w0 * 1.86), PLANK);
        for z in [-1.0, 1.0] {
            m.block(v(x0 + 0.22, top0 + 0.14, z * w0 * 0.95), v(0.46, 0.04, 0.025), GOLD);
        }
        m.block(v(x0 + 0.005, top0 + 0.14, 0.0), v(0.025, 0.04, w0 * 1.92), GOLD);
        for z in [-0.16, 0.0, 0.16] {
            m.cuboid(v(x0 - 0.002, top0 + 0.04, z), v(0.01, 0.06, 0.07), LAMP, Finish::Glow);
        }
        m.ball(v(x0 - 0.04, top0 + 0.28, 0.0), Vec3::splat(0.04), 8, LAMP, Finish::Glow);
        m.rod(v(x0, top0 + 0.16, 0.0), v(x0 - 0.04, top0 + 0.26, 0.0), 0.01, WOOD_DARK);
        let (xf, wf, topf, _) = h.at(0.82);
        m.block(v(xf, topf - 0.03, 0.0), v(0.3, 0.1, wf * 1.8), HULL_TAR);
        // Three masts, with a course and a topsail on each, and the lowest course in the country's color.
        let (_, _, mid, _) = h.at(0.5);
        let deck = mid - 0.025;
        let masts = [(-0.62, 1.3), (0.02, 1.62), (0.6, 1.38)];
        for (k, (x, height)) in masts.into_iter().enumerate() {
            m.frustum(v(x, deck, 0.0), v(x, deck + height, 0.0), 0.03, 0.018, 6, 0.0, WOOD_DARK, Finish::Matte);
            let width = if k == 1 { 0.95 } else { 0.8 };
            let low = deck + height * 0.62;
            square_sail(m, x, low, width, height * 0.36, time, |_, w| if (0.35..0.65).contains(&w) { country } else { CLOTH });
            square_sail(m, x, deck + height * 0.95, width * 0.75, height * 0.3, time + 0.7, |_, _| CLOTH);
            if k == 1 {
                m.ring(v(x, deck + height * 0.64, 0.0), 0.03, 0.09, 0.04, 10, WOOD_DARK, Finish::Matte);
                pennant(m, v(x, deck + height + 0.01, 0.0), 0.55, country, time);
            }
        }
        // The bowsprit, with a jib to the foremast.
        let (xb, _, topb, _) = h.at(1.0);
        m.rod(v(xb - 0.05, topb, 0.0), v(xb + 0.4, topb + 0.18, 0.0), 0.016, WOOD_DARK);
        jib(m, v(0.6, deck + 1.2, 0.0), v(xb + 0.38, topb + 0.18, 0.0), v(0.66, deck + 0.12, 0.0));
    } else {
        let h = Hull { length: 1.7, beam: 0.52, height: 0.2, draft: 0.14, wale: 0.06, lift: 0.0, along: 0.0 };
        hull(m, &h, country, HULL_WOOD, HULL_LOW, 1.0, true);
        let (_, _, mid, _) = h.at(0.5);
        let deck = mid - 0.025;
        // A small cabin aft.
        let (xs, ws, tops, _) = h.at(0.08);
        m.block(v(xs + 0.16, tops - 0.03, 0.0), v(0.28, 0.1, ws * 1.5), HULL_WOOD);
        m.block(v(xs + 0.16, tops + 0.07, 0.0), v(0.3, 0.02, ws * 1.6), WOOD_DARK);
        // One mast with a great striped sail.
        let (x, height) = (0.05, 1.18);
        m.frustum(v(x, deck, 0.0), v(x, deck + height, 0.0), 0.028, 0.017, 6, 0.0, WOOD_DARK, Finish::Matte);
        square_sail(m, x, deck + height * 0.9, 0.82, height * 0.62, time, |u, _| if ((u * 6.0) as usize) % 2 == 0 { country } else { CLOTH });
        pennant(m, v(x, deck + height + 0.01, 0.0), 0.45, country, time);
        let (xb, _, topb, _) = h.at(1.0);
        m.rod(v(xb - 0.04, topb, 0.0), v(xb + 0.3, topb + 0.14, 0.0), 0.014, WOOD_DARK);
        jib(m, v(x, deck + height * 0.95, 0.0), v(xb + 0.28, topb + 0.14, 0.0), v(x + 0.12, deck + 0.1, 0.0));
    }
}

/// A three-cornered sail from `head`, at the mast, to `tack`, at the end of the bowsprit, and down to `clew`.
fn jib(m: &mut Model, head: Vec3, tack: Vec3, clew: Vec3) {
    let belly = Vec3::Z * 0.04;
    let mid = (head + tack + clew) / 3.0 + belly;
    for (a, b) in [(head, tack), (tack, clew), (clew, head)] {
        m.tri(a, b, mid, CLOTH, Finish::Matte);
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// Gatherers.

fn woodcutter(m: &mut Model, country: Color) {
    let f = plot(m, Look::Woodcutter.reach(), GRASS, country);
    pine(m, v(-0.45, f, -0.42), 1.0);
    pine(m, v(-0.58, f, 0.34), 0.82);
    pine(m, v(0.12, f, -0.6), 0.7);
    pine(m, v(-0.05, f, 0.0), 0.55);
    // The woodcutter's hut, with a roof of thatch.
    house(m, v(0.42, f, 0.42), v(0.5, 0.28, 0.36), 0.0, WOOD, THATCH, 0.22);
    // A pile of logs, and a stump with an axe in it.
    for (x, y, z) in [(0.38, 0.06, -0.08), (0.52, 0.06, -0.08), (0.45, 0.17, -0.08)] {
        log(m, v(x, f + y, z - 0.22), v(x, f + y, z + 0.18), 0.06);
    }
    m.column(v(0.7, f, -0.5), 0.09, 0.085, 0.1, 10, BARK, Finish::Matte);
    m.column(v(0.7, f + 0.1, -0.5), 0.08, 0.08, 0.004, 10, PLANK, Finish::Matte);
    m.rod(v(0.7, f + 0.1, -0.5), v(0.78, f + 0.32, -0.5), 0.012, WOOD);
    m.cuboid(v(0.7, f + 0.115, -0.5), v(0.09, 0.05, 0.016), STEEL, Finish::Metal);
}

fn sawmill(m: &mut Model, country: Color, moment: Moment) {
    let f = plot(m, Look::Sawmill.reach(), DIRT, country);
    // The mill race, between stone banks, that turns the wheel.
    m.block(v(0.0, f, 0.8), v(2.2, 0.025, 0.34), WATER);
    m.cuboid(v(0.0, f + 0.012, 0.8), v(2.2, 0.025, 0.34), WATER, Finish::Gloss);
    for z in [0.62, 0.98] {
        m.block(v(0.0, f, z), v(2.2, 0.06, 0.05), STONE_DARK);
    }
    // The mill, with a red roof.
    house(m, v(-0.15, f, 0.15), v(1.05, 0.48, 0.72), 0.0, WOOD, ROOF_RED, 0.36);
    for x in [-0.55, -0.15, 0.25] {
        m.cuboid(v(-0.15 + x * 0.9, f + 0.3, 0.515), v(0.12, 0.12, 0.012), COAL, Finish::Matte);
    }
    // The water wheel, which turns while the mill is working.
    let angle = if moment.busy { -moment.time * 1.2 } else { 0.3 };
    water_wheel(m, v(-0.2, f + 0.3, 0.8), 0.3, 0.12, angle);
    // A saw bench in front with a log on it and the blade, and stacks of planks and a pile of logs.
    m.block(v(0.85, f, 0.15), v(0.62, 0.16, 0.22), WOOD_DARK);
    log(m, v(0.6, f + 0.21, 0.15), v(1.1, f + 0.21, 0.15), 0.05);
    m.frustum(v(0.78, f + 0.2, 0.135), v(0.78, f + 0.2, 0.165), 0.14, 0.14, 16, 0.0, STEEL, Finish::Metal);
    planks(m, v(0.8, f, -0.72), 0.6, 4);
    planks(m, v(0.85, f, -0.3), 0.55, 3);
    for (x, y, z) in [(-0.05, 0.06, -0.72), (-0.05, 0.06, -0.86), (-0.05, 0.17, -0.79)] {
        log(m, v(x - 0.3, f + y, z), v(x + 0.32, f + y, z), 0.06);
    }
    pine(m, v(-0.92, f, -0.4), 0.9);
    pine(m, v(-0.55, f, -0.85), 0.7);
}

/// A water wheel with its middle at `centre`, turning about z, `radius` round and `width` across, turned to `angle`.
fn water_wheel(m: &mut Model, centre: Vec3, radius: f32, width: f32, angle: f32) {
    m.frustum(centre - Vec3::Z * width * 1.2, centre + Vec3::Z * width * 0.9, 0.025, 0.025, 6, 0.0, WOOD_DARK, Finish::Matte);
    let at = |a: f32, r: f32, z: f32| centre + v(a.cos() * r, a.sin() * r, z);
    for side in [-0.5, 0.5] {
        let z = width * side;
        for k in 0..12 {
            let (a0, a1) = (angle + TAU * k as f32 / 12.0, angle + TAU * (k + 1) as f32 / 12.0);
            m.frustum(at(a0, radius, z), at(a1, radius, z), 0.016, 0.016, 4, 0.0, WOOD_DARK, Finish::Matte);
        }
        for k in 0..4 {
            let a = angle + FRAC_PI_2 * k as f32;
            m.frustum(at(a, radius, z), at(a + PI, radius, z), 0.011, 0.011, 4, 0.0, WOOD_DARK, Finish::Matte);
        }
    }
    for k in 0..10 {
        let a = angle + TAU * k as f32 / 10.0;
        let (out, along) = (v(a.cos(), a.sin(), 0.0), v(-a.sin(), a.cos(), 0.0));
        m.oriented_box(at(a, radius * 0.88, 0.0), Mat3::from_cols(out, along, Vec3::Z), v(radius * 0.34, 0.022, width * 1.05), WOOD, Finish::Matte);
    }
}

fn stonecutter(m: &mut Model, country: Color) {
    let f = plot(m, Look::Stonecutter.reach(), DIRT, country);
    // Rough boulders, as they come from the ground.
    for (x, z, r, tone) in [(-0.4, -0.36, v(0.4, 0.32, 0.36), 1.0), (-0.56, 0.32, v(0.32, 0.26, 0.3), 0.88), (0.08, -0.62, v(0.26, 0.19, 0.22), 0.94)] {
        m.ball(v(x, f + r.y * 0.6, z), r, 7, shade(STONE, tone), Finish::Matte);
    }
    // A hoist of three poles with a block hanging from it, being lifted.
    let apex = v(0.1, f + 1.0, 0.05);
    for k in 0..3 {
        let a = TAU * k as f32 / 3.0 + 0.4;
        m.frustum(v(0.1 + a.cos() * 0.42, f, 0.05 + a.sin() * 0.42), apex, 0.022, 0.016, 5, 0.0, WOOD, Finish::Matte);
    }
    m.ball(apex, Vec3::splat(0.035), 6, WOOD_DARK, Finish::Matte);
    m.rod(apex, v(0.1, f + 0.46, 0.05), 0.006, LEATHER);
    m.block(v(0.1, f + 0.3, 0.05), v(0.2, 0.16, 0.2), STONE_LIGHT);
    // Blocks that have been cut, stacked.
    for (x, y, z) in [(0.5, 0.0, 0.38), (0.5, 0.0, 0.62), (0.74, 0.0, 0.5), (0.56, 0.2, 0.5)] {
        m.block(v(x, f + y, z), v(0.21, 0.2, 0.21), STONE_LIGHT);
    }
    m.block_turned(v(0.6, f, -0.3), v(0.28, 0.14, 0.2), 0.3, shade(STONE_LIGHT, 0.92));
}

fn quarry(m: &mut Model, country: Color) {
    let f = plot(m, Look::Quarry.reach(), GRAVEL, country);
    let inner = 1.1;
    // The face of the quarry, cut in steps into the hill at the back, with the grass of the hill on top.
    let steps = [(0.0, 0.3, inner, 0.92), (-0.42, 0.62, inner * 0.85, 1.0), (-0.78, 0.98, inner * 0.62, 0.86)];
    for (front, height, wide, tone) in steps {
        let depth = front + inner;
        m.block(v(front - depth * 0.5, f, -inner + wide), v(depth, height, wide * 2.0), shade(STONE_LIGHT, tone));
    }
    let (front, height, wide, _) = steps[2];
    let depth = front + inner;
    m.block(v(front - depth * 0.5, f + height, -inner + wide), v(depth + 0.01, 0.04, wide * 2.0 + 0.01), GRASS);
    // A derrick at the foot of the face, lifting a block.
    let foot = v(0.42, f, -0.52);
    let top = foot + Vec3::Y * 1.42;
    m.frustum(foot, top, 0.03, 0.022, 6, 0.0, WOOD, Finish::Matte);
    let tip = v(-0.25, f + 1.2, -0.15);
    m.frustum(foot + Vec3::Y * 0.25, tip, 0.024, 0.018, 6, 0.0, WOOD, Finish::Matte);
    m.rod(top, tip, 0.006, LEATHER);
    m.rod(tip, v(-0.25, f + 0.62, -0.15), 0.006, LEATHER);
    m.block(v(-0.25, f + 0.46, -0.15), v(0.2, 0.16, 0.2), STONE_LIGHT);
    // Cut blocks, stacked, and a cart.
    for (x, y, z) in [(0.62, 0.0, 0.45), (0.86, 0.0, 0.45), (0.62, 0.0, 0.7), (0.86, 0.0, 0.7), (0.74, 0.2, 0.57)] {
        m.block(v(x, f + y, z), v(0.22, 0.2, 0.22), shade(STONE_LIGHT, 0.97));
    }
    m.block(v(0.62, f + 0.08, -0.05), v(0.4, 0.12, 0.26), WOOD);
    for z in [-0.19, 0.09] {
        m.frustum(v(0.62, f + 0.08, z - 0.015), v(0.62, f + 0.08, z + 0.015), 0.08, 0.08, 12, 0.0, WOOD_DARK, Finish::Matte);
    }
    m.block(v(0.62, f + 0.2, -0.05), v(0.18, 0.14, 0.18), STONE_LIGHT);
}

/// A headframe over a shaft, with its foot at `foot`: four legs that lean in to a wheel at the top, braced, and a strut behind.
fn headframe(m: &mut Model, foot: Vec3, height: f32, wheel_angle: f32) {
    let (base, top) = (0.24, 0.07);
    let corner = |sx: f32, sz: f32, y: f32| {
        let s = base + (top - base) * (y / height);
        foot + v(sx * s, y, sz * s)
    };
    let corners = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)];
    for (sx, sz) in corners {
        m.frustum(corner(sx, sz, 0.0), corner(sx, sz, height), 0.024, 0.018, 5, 0.0, WOOD_DARK, Finish::Matte);
    }
    for y in [0.4, 0.8, 1.15] {
        let y = y * height / 1.4;
        for k in 0..4 {
            let ((ax, az), (bx, bz)) = (corners[k], corners[(k + 1) % 4]);
            m.frustum(corner(ax, az, y), corner(bx, bz, y), 0.012, 0.012, 4, 0.0, WOOD_DARK, Finish::Matte);
        }
    }
    m.block(foot + Vec3::Y * height, v(0.2, 0.05, 0.2), WOOD_DARK);
    let hub = foot + v(0.0, height + 0.16, 0.0);
    m.frustum(hub - Vec3::Z * 0.025, hub + Vec3::Z * 0.025, 0.15, 0.15, 16, 0.0, IRON, Finish::Metal);
    m.frustum(hub - Vec3::Z * 0.03, hub + Vec3::Z * 0.03, 0.035, 0.035, 8, 0.0, COAL, Finish::Matte);
    for k in 0..3 {
        let a = wheel_angle + TAU * k as f32 / 6.0;
        m.frustum(hub + v(a.cos(), a.sin(), 0.0) * 0.14 + Vec3::Z * 0.03, hub - v(a.cos(), a.sin(), 0.0) * 0.14 + Vec3::Z * 0.03, 0.008, 0.008, 4, 0.0, COAL, Finish::Matte);
    }
    m.frustum(foot + v(-0.62, 0.0, 0.0), hub - v(0.02, 0.08, 0.0), 0.022, 0.018, 5, 0.0, WOOD_DARK, Finish::Matte);
}

fn iron_mine(m: &mut Model, country: Color, moment: Moment) {
    let f = plot(m, Look::IronMine.reach(), DIRT, country);
    // The hill the mine goes into, of dark rock.
    for (x, z, r, tone) in [(-0.6, -0.18, v(0.62, 0.62, 0.72), 1.0), (-0.72, 0.6, v(0.42, 0.4, 0.44), 0.88), (-0.25, -0.78, v(0.42, 0.32, 0.36), 0.94)] {
        m.dome(v(x, f, z), r, 9, shade(ROCK, tone), Finish::Matte);
    }
    // The way in, framed in timber, and rails out of it with a cart of ore on them.
    let mouth = v(-0.05, f, -0.12);
    m.cuboid(mouth + v(0.0, 0.17, 0.0), v(0.12, 0.32, 0.26), COAL, Finish::Matte);
    for z in [-0.16, 0.16] {
        m.block(mouth + v(0.04, 0.0, z), v(0.05, 0.36, 0.05), WOOD);
    }
    m.block(mouth + v(0.04, 0.36, 0.0), v(0.07, 0.06, 0.42), WOOD);
    for z in [-0.06, 0.06] {
        m.block(v(0.5, f, -0.12 + z), v(0.95, 0.025, 0.018), STEEL);
    }
    for k in 0..7 {
        m.block(v(0.08 + 0.14 * k as f32, f, -0.12), v(0.04, 0.012, 0.2), WOOD_DARK);
    }
    let cart = v(0.56, f + 0.03, -0.12);
    for x in [-0.08, 0.08] {
        m.frustum(cart + v(x, 0.03, -0.11), cart + v(x, 0.03, 0.11), 0.045, 0.045, 10, 0.0, COAL, Finish::Matte);
    }
    m.frustum(cart + Vec3::Y * 0.03, cart + Vec3::Y * 0.18, 0.14, 0.18, 4, FRAC_PI_4, IRON, Finish::Metal);
    m.dome(cart + Vec3::Y * 0.18, v(0.15, 0.09, 0.15), 7, RUST, Finish::Matte);
    // The headframe over the shaft, which winds while the mine is working.
    let wheel = if moment.busy { moment.time * 2.0 } else { 0.0 };
    headframe(m, v(0.6, f, 0.55), 1.4, wheel);
    // Heaps of rust red ore.
    m.column(v(0.72, f, -0.72), 0.36, 0.0, 0.38, 12, RUST, Finish::Matte);
    m.column(v(0.3, f, -0.82), 0.24, 0.0, 0.24, 10, shade(RUST, 0.85), Finish::Matte);
}

fn copper_mine(m: &mut Model, country: Color, moment: Moment) {
    let f = plot(m, Look::CopperMine.reach(), RED_EARTH, country);
    // The smelter: a stone furnace with a fire in its mouth, and a tall brick chimney that smokes while it is working.
    let furnace = v(-0.48, f, -0.38);
    m.frustum(furnace, furnace + Vec3::Y * 0.46, 0.38, 0.3, 6, 0.0, STONE_DARK, Finish::Matte);
    m.cuboid(furnace + v(0.33, 0.15, 0.0), v(0.03, 0.15, 0.16), FIRE, Finish::Glow);
    let chimney_top = furnace + Vec3::Y * 1.62;
    m.frustum(furnace + Vec3::Y * 0.46, chimney_top, 0.13, 0.1, 10, 0.0, BRICK, Finish::Matte);
    m.column(chimney_top - Vec3::Y * 0.06, 0.125, 0.125, 0.07, 10, shade(BRICK, 0.7), Finish::Matte);
    if moment.busy {
        puffs(m, chimney_top + Vec3::Y * 0.05, moment.time, 4, 0.6, 0.08, SMOKE);
    }
    // Heaps of green ore.
    m.column(v(0.55, f, -0.62), 0.38, 0.0, 0.36, 12, MALACHITE, Finish::Matte);
    m.column(v(0.12, f, -0.86), 0.24, 0.0, 0.22, 10, shade(MALACHITE, 0.82), Finish::Matte);
    m.column(v(0.82, f, -0.2), 0.2, 0.0, 0.18, 10, shade(MALACHITE, 1.12), Finish::Matte);
    // Stacks of bright ingots.
    for (sx, sz) in [(0.55, 0.42), (0.12, 0.7)] {
        for layer in 0..3 {
            let count = 3 - layer;
            for k in 0..count {
                let offset = (k as f32 - (count - 1) as f32 * 0.5) * 0.11;
                let (dx, dz) = if layer % 2 == 0 { (offset, 0.0) } else { (0.0, offset) };
                let size = if layer % 2 == 0 { v(0.09, 0.055, 0.24) } else { v(0.24, 0.055, 0.09) };
                m.cuboid(v(sx + dx, f + 0.03 + layer as f32 * 0.058, sz + dz), size, COPPER, Finish::Metal);
            }
        }
    }
    // A shed with a roof of green copper, over the anvil.
    let shed = v(-0.5, f, 0.52);
    for (x, z) in [(-0.25, -0.2), (0.25, -0.2), (0.25, 0.2), (-0.25, 0.2)] {
        m.rod(shed + v(x, 0.0, z), shed + v(x, 0.42, z), 0.02, WOOD);
    }
    m.roof(shed + Vec3::Y * 0.42, 0.66, 0.56, 0.22, 0.0, PATINA, WOOD_DARK);
    m.block(shed + v(0.0, 0.0, 0.0), v(0.12, 0.12, 0.1), WOOD_DARK);
    m.block(shed + v(0.0, 0.12, 0.0), v(0.16, 0.05, 0.08), IRON);
}

fn sulphur_pit(m: &mut Model, country: Color, moment: Moment) {
    let f = plot(m, Look::SulphurPit.reach(), ASH, country);
    // The crater: a ring of yellow mounds round a vent that steams, and burns blue while it is worked.
    let middle = v(-0.22, f, -0.1);
    for k in 0..10 {
        let a = TAU * k as f32 / 10.0;
        let tall = 0.2 + 0.08 * ((k * 3) % 4) as f32;
        m.dome(middle + v(a.cos() * 0.6, 0.0, a.sin() * 0.6), v(0.27, tall, 0.27), 10, shade(SULPHUR, 0.85 + 0.06 * (k % 3) as f32), Finish::Matte);
    }
    m.column(middle, 0.42, 0.42, 0.012, 16, c(0.42, 0.36, 0.12), Finish::Matte);
    m.column(middle + Vec3::Y * 0.012, 0.16, 0.16, 0.006, 12, COAL, Finish::Matte);
    if moment.busy {
        let flicker = 1.0 + 0.15 * (moment.time * 11.0).sin();
        m.column(middle + Vec3::Y * 0.018, 0.09, 0.0, 0.18 * flicker, 8, BLUE_FLAME, Finish::Glow);
        puffs(m, middle + Vec3::Y * 0.12, moment.time, 4, 0.9, 0.11, STEAM);
    }
    // A plank walk into it, heaps of sulphur dug out, and baskets of it.
    m.block_turned(v(0.42, f + 0.06, 0.24), v(0.6, 0.03, 0.14), -0.5, PLANK);
    m.column(v(0.76, f, 0.68), 0.3, 0.0, 0.3, 12, SULPHUR, Finish::Matte);
    m.column(v(0.82, f, -0.62), 0.24, 0.0, 0.24, 12, shade(SULPHUR, 1.05), Finish::Matte);
    for (x, z) in [(0.36, 0.86), (0.54, 0.98), (0.2, 0.98)] {
        m.column(v(x, f, z), 0.075, 0.09, 0.11, 10, THATCH, Finish::Matte);
        m.dome(v(x, f + 0.11, z), v(0.08, 0.05, 0.08), 8, SULPHUR, Finish::Matte);
    }
}

fn oil_well(m: &mut Model, country: Color, moment: Moment) {
    let f = plot(m, Look::OilWell.reach(), OILY, country);
    // The derrick: a tall tower of timber, braced at each level.
    let foot = v(-0.42, f, -0.32);
    let (height, base, top) = (1.9, 0.3, 0.06);
    let corner = |sx: f32, sz: f32, y: f32| foot + v(sx * (base + (top - base) * y / height), y, sz * (base + (top - base) * y / height));
    let corners = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)];
    for (sx, sz) in corners {
        m.frustum(corner(sx, sz, 0.0), corner(sx, sz, height), 0.024, 0.016, 5, 0.0, WOOD_DARK, Finish::Matte);
    }
    let levels = [0.0, 0.34, 0.72, 1.08, 1.44];
    for pair in levels.windows(2) {
        for k in 0..4 {
            let ((ax, az), (bx, bz)) = (corners[k], corners[(k + 1) % 4]);
            m.frustum(corner(ax, az, pair[1]), corner(bx, bz, pair[1]), 0.011, 0.011, 4, 0.0, WOOD_DARK, Finish::Matte);
            m.frustum(corner(ax, az, pair[0]), corner(bx, bz, pair[1]), 0.009, 0.009, 4, 0.0, WOOD_DARK, Finish::Matte);
        }
    }
    m.block(foot + Vec3::Y * 0.34, v(0.5, 0.025, 0.5), WOOD);
    m.block(foot + Vec3::Y * height, v(0.17, 0.08, 0.17), WOOD_DARK);
    m.rod(foot + Vec3::Y * height, foot + Vec3::Y * 0.36, 0.006, IRON);
    // The pump, painted red, which nods while the well is working.
    let pump = v(0.5, f, 0.4);
    m.block(pump, v(0.82, 0.05, 0.16), IRON);
    let pivot = pump + Vec3::Y * 0.5;
    for z in [-0.06, 0.06] {
        m.frustum(pump + v(-0.12, 0.05, z), pivot + Vec3::Z * z, 0.016, 0.016, 5, 0.0, PUMP_RED, Finish::Metal);
        m.frustum(pump + v(0.12, 0.05, z), pivot + Vec3::Z * z, 0.016, 0.016, 5, 0.0, PUMP_RED, Finish::Metal);
    }
    let crank = if moment.busy { moment.time * 1.8 } else { 0.0 };
    let nod = crank.sin() * 0.28;
    let beam = Mat3::from_rotation_z(nod);
    m.oriented_box(pivot, beam, v(0.82, 0.055, 0.06), PUMP_RED, Finish::Metal);
    let head = pivot + beam * v(0.42, -0.04, 0.0);
    m.oriented_box(head, beam, v(0.08, 0.2, 0.07), PUMP_RED, Finish::Metal);
    let tail = pivot + beam * v(-0.4, 0.0, 0.0);
    let hub = pump + v(-0.32, 0.17, 0.0);
    let pin = hub + v(crank.cos(), crank.sin(), 0.0) * 0.08;
    for z in [-0.1, 0.1] {
        m.frustum(hub + Vec3::Z * (z - 0.015), hub + Vec3::Z * (z + 0.015), 0.11, 0.11, 12, 0.0, IRON, Finish::Metal);
    }
    m.rod(tail, pin + Vec3::Z * 0.1, 0.012, IRON);
    let rod_top = head + beam * v(0.04, -0.1, 0.0);
    m.rod(rod_top, v(rod_top.x, pump.y + 0.06, rod_top.z), 0.006, STEEL);
    m.column(v(rod_top.x, pump.y, rod_top.z), 0.035, 0.03, 0.08, 8, IRON, Finish::Metal);
    // A black tank, a pool of oil, and barrels.
    let tank = v(0.55, f, -0.55);
    m.column(tank, 0.33, 0.33, 0.44, 18, TANK, Finish::Gloss);
    m.column(tank + Vec3::Y * 0.44, 0.34, 0.2, 0.06, 18, shade(TANK, 1.25), Finish::Gloss);
    m.column(tank + Vec3::Y * 0.2, 0.335, 0.335, 0.03, 18, PUMP_RED, Finish::Metal);
    m.column(v(-0.55, f, 0.62), 0.32, 0.32, 0.008, 16, OIL, Finish::Gloss);
    m.column(v(-0.55, f, 0.62), 0.38, 0.38, 0.004, 16, shade(OILY, 0.6), Finish::Matte);
    for (x, z) in [(0.0, 0.88), (0.16, 0.88), (0.08, 0.74), (-0.08, 0.74)] {
        m.column(v(x, f, z), 0.07, 0.07, 0.16, 10, TANK, Finish::Gloss);
        m.column(v(x, f + 0.06, z), 0.073, 0.073, 0.025, 10, PUMP_RED, Finish::Matte);
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// Shipyards.

/// A ship being built on the stocks along x: ribs from end to end, planked from the stern to `planked` of
/// its length.
fn ship_on_the_stocks(m: &mut Model, hull_size: &Hull, ribs: usize, planked: f32) {
    hull(m, hull_size, PLANK, WOOD, WOOD, planked, false);
    let (x0, _, _, keel0) = hull_size.at(0.0);
    let (x1, _, _, keel1) = hull_size.at(1.0);
    m.frustum(v(x0, keel0, 0.0), v(x1, keel1 + 0.06, 0.0), 0.03, 0.03, 4, FRAC_PI_4, WOOD_DARK, Finish::Matte);
    m.frustum(v(x1, keel1, 0.0), v(x1 + 0.05, hull_size.at(1.0).2 + 0.05, 0.0), 0.022, 0.02, 4, FRAC_PI_4, WOOD_DARK, Finish::Matte);
    for k in 0..ribs {
        let t = 0.05 + 0.88 * k as f32 / (ribs - 1) as f32;
        for side in [-1.0, 1.0] {
            let s = hull_size.section(t, side);
            for pair in s.windows(2) {
                m.frustum(pair[0] * v(1.0, 1.0, 1.04), pair[1] * v(1.0, 1.0, 1.04), 0.014, 0.014, 4, 0.0, PLANK, Finish::Matte);
            }
        }
    }
}

/// A crane standing at `foot`, `height` high, its jib reaching out to `tip`, with a load hanging from it.
fn crane(m: &mut Model, foot: Vec3, height: f32, tip: Vec3) {
    m.block(foot, v(0.2, 0.06, 0.2), STONE_DARK);
    let top = foot + Vec3::Y * height;
    m.frustum(foot, top, 0.035, 0.026, 6, 0.0, WOOD_DARK, Finish::Matte);
    for (dx, dz) in [(0.12, 0.0), (-0.12, 0.0), (0.0, 0.12)] {
        m.frustum(foot + v(dx, 0.0, dz), foot + Vec3::Y * height * 0.4, 0.014, 0.014, 4, 0.0, WOOD_DARK, Finish::Matte);
    }
    let jib_foot = foot + Vec3::Y * height * 0.72;
    m.frustum(jib_foot, tip, 0.024, 0.016, 5, 0.0, WOOD, Finish::Matte);
    m.rod(top, tip, 0.006, LEATHER);
    let hook = tip - Vec3::Y * (tip.y - foot.y) * 0.45;
    m.rod(tip, hook, 0.005, LEATHER);
    m.block(hook - Vec3::Y * 0.1, v(0.26, 0.1, 0.08), PLANK);
}

fn shipyard(m: &mut Model, large: bool, country: Color) {
    let half = Look::Shipyard { large }.reach();
    let f = plot(m, half, SAND, country);
    let inner = half * 0.88;
    // The slip, open to the water, with a stone quay along it.
    let edge = inner * 0.42;
    m.cuboid(v((edge + inner) * 0.5, f + 0.004, 0.0), v(inner - edge, 0.012, inner * 2.0), WATER, Finish::Gloss);
    m.block(v(edge, f, 0.0), v(0.06, 0.05, inner * 2.0), STONE);
    for z in [-1.0, 1.0] {
        m.block(v((edge + inner) * 0.5, f, z * (inner - 0.04)), v(inner - edge, 0.05, 0.08), STONE);
    }
    // The slipway: two great timbers running down into the water.
    let (length, beam) = if large { (2.25, 0.62) } else { (1.65, 0.5) };
    let middle_x = edge - length * 0.38;
    for z in [-0.13, 0.13] {
        m.block(v(middle_x + 0.12, f, z), v(length * 1.05 + 0.3, 0.05, 0.07), WOOD_DARK);
    }
    // The ship on the stocks, ribbed from end to end and planked from the stern.
    let h = Hull { length, beam, height: if large { 0.3 } else { 0.22 }, draft: if large { 0.2 } else { 0.15 }, wale: 0.05, lift: f + 0.09 + if large { 0.2 } else { 0.15 }, along: middle_x };
    ship_on_the_stocks(m, &h, if large { 10 } else { 7 }, if large { 0.6 } else { 0.42 });
    // Props under the keel, and scaffolding along one side, with a walk on it.
    for k in 0..4 {
        let x = middle_x - length * 0.36 + length * 0.24 * k as f32;
        m.block(v(x, f, 0.0), v(0.06, h.lift - f - h.draft * 0.9, 0.06), WOOD_DARK);
    }
    let side = beam * 0.5 + 0.12;
    for k in 0..5 {
        let x = middle_x - length * 0.4 + length * 0.2 * k as f32;
        m.rod(v(x, f, -side), v(x, h.lift + h.height + 0.12, -side), 0.014, WOOD);
    }
    m.block(v(middle_x, h.lift + h.height * 0.55, -side), v(length * 0.85, 0.02, 0.12), PLANK);
    // A crane over it, and, in a large yard, a second crane, a storehouse and a stack of timber.
    crane(m, v(middle_x - 0.1, f, inner - 0.22), 1.45, v(middle_x + 0.1, f + 1.2, 0.05));
    planks(m, v(-inner + 0.36, f, -inner + 0.25), 0.55, 4);
    if large {
        crane(m, v(middle_x + 0.6, f, -inner + 0.18), 1.6, v(middle_x + 0.55, f + 1.35, -0.05));
        house(m, v(-inner + 0.36, f, inner - 0.36), v(0.62, 0.36, 0.5), FRAC_PI_2, STONE, ROOF_SLATE, 0.26);
        planks(m, v(-inner + 0.36, f, -inner + 0.62), 0.55, 3);
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// Barracks.

/// A stretch of wall of `size` standing on `foot`, along whichever of x and z is the longer, with battlements along its top.
fn wall(m: &mut Model, foot: Vec3, size: Vec3, color: Color) {
    m.block(foot, size, color);
    let along_x = size.x > size.z;
    let (length, thick) = (size.x.max(size.z), size.x.min(size.z));
    let count = (length / 0.17).floor() as usize;
    for k in (0..count).step_by(2) {
        let at = -length * 0.5 + (k as f32 + 0.5) * length / count as f32;
        let offset = if along_x { v(at, 0.0, 0.0) } else { v(0.0, 0.0, at) };
        m.block(foot + offset + Vec3::Y * size.y, if along_x { v(0.085, 0.08, thick * 1.1) } else { v(thick * 1.1, 0.08, 0.085) }, shade(color, 1.06));
    }
}

fn barracks(m: &mut Model, country: Color, time: f32) {
    let half = Look::Barracks.reach();
    let f = plot(m, half, GRAVEL, country);
    let w = half * 0.76;
    let (thick, tall) = (0.12, 0.4);
    // The curtain walls, with battlements, and a gap in the front one for the gate.
    let gate = 0.2;
    let walls = [
        (v(-w, f, 0.0), v(thick, tall, w * 2.0)),
        (v(0.0, f, -w), v(w * 2.0, tall, thick)),
        (v(0.0, f, w), v(w * 2.0, tall, thick)),
        (v(w, f, -(w + gate) * 0.5), v(thick, tall, w - gate)),
        (v(w, f, (w + gate) * 0.5), v(thick, tall, w - gate)),
    ];
    for (foot, size) in walls {
        wall(m, foot, size, STONE);
    }
    // The gatehouse, and its gate.
    for z in [-1.0, 1.0] {
        m.block(v(w, f, z * (gate + 0.06)), v(0.24, 0.58, 0.16), STONE);
        m.block(v(w, f + 0.58, z * (gate + 0.06)), v(0.27, 0.05, 0.19), shade(STONE, 0.85));
    }
    m.block(v(w, f + 0.36, 0.0), v(0.2, 0.18, gate * 2.0 + 0.04), STONE);
    m.cuboid(v(w + 0.02, f + 0.18, 0.0), v(0.03, 0.36, gate * 2.0), WOOD_DARK, Finish::Matte);
    // A round tower at each corner, with a pointed roof of the country's color.
    for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
        let at = v(sx * w, f, sz * w);
        m.column(at, 0.22, 0.2, 0.66, 14, STONE, Finish::Matte);
        m.column(at + Vec3::Y * 0.62, 0.24, 0.24, 0.07, 14, shade(STONE, 0.88), Finish::Matte);
        m.column(at + Vec3::Y * 0.69, 0.25, 0.0, 0.46, 14, country, Finish::Gloss);
        m.cuboid(at + v(sx * -0.1, 0.42, sz * 0.15), v(0.03, 0.1, 0.03), COAL, Finish::Matte);
    }
    // The hall inside, with a slate roof, and the training ground in front: straw men to fight.
    house(m, v(-0.32, f, 0.0), v(0.5, 0.36, 0.95), FRAC_PI_2, STONE_LIGHT, ROOF_SLATE, 0.32);
    for z in [0.18, 0.42] {
        let post = v(0.4, f, z);
        m.rod(post, post + Vec3::Y * 0.36, 0.015, WOOD);
        m.rod(post + v(0.0, 0.28, -0.08), post + v(0.0, 0.28, 0.08), 0.012, WOOD);
        m.ball(post + Vec3::Y * 0.31, v(0.06, 0.09, 0.06), 8, THATCH, Finish::Matte);
        m.ball(post + Vec3::Y * 0.42, Vec3::splat(0.045), 8, THATCH, Finish::Matte);
    }
    // The great banner.
    flag(m, v(0.32, f, -0.38), 1.55, country, time, true, 1.25);
}

// ---------------------------------------------------------------------------------------------------------------------------
// Forts.

/// A square of walls reaching `w` out from the middle, `tall` high, standing at height `f`, with a gap of `gate` on each side of
/// the middle of the +x side.
fn walls_round(m: &mut Model, f: f32, w: f32, tall: f32, gate: f32, color: Color) {
    let thick = 0.12;
    wall(m, v(-w, f, 0.0), v(thick, tall, w * 2.0), color);
    wall(m, v(0.0, f, -w), v(w * 2.0, tall, thick), color);
    wall(m, v(0.0, f, w), v(w * 2.0, tall, thick), color);
    wall(m, v(w, f, -(w + gate) * 0.5), v(thick, tall, w - gate), color);
    wall(m, v(w, f, (w + gate) * 0.5), v(thick, tall, w - gate), color);
    m.cuboid(v(w, f + tall * 0.4, 0.0), v(0.05, tall * 0.8, gate * 2.0), WOOD_DARK, Finish::Matte);
}

/// A square tower `side` across and `tall` high standing on `foot`, with a battlement at each corner of its top.
fn square_tower(m: &mut Model, foot: Vec3, side: f32, tall: f32, color: Color) {
    m.block(foot, v(side, tall, side), color);
    m.block(foot + Vec3::Y * tall, v(side * 1.14, 0.06, side * 1.14), shade(color, 0.85));
    let reach = side * 0.45;
    for (x, z) in [(-reach, -reach), (reach, -reach), (-reach, reach), (reach, reach)] {
        m.block(foot + v(x, tall + 0.06, z), v(side * 0.24, 0.09, side * 0.24), color);
    }
}

fn fort(m: &mut Model, size: FortSize, country: Color, time: f32) {
    let half = Look::Fort { size }.reach();
    let f = plot(m, half, if size == FortSize::Small { DIRT } else { GRAVEL }, country);
    match size {
        FortSize::Small => {
            // A palisade of pointed stakes on a footing of stones, with a gate in the +x side.
            let w = half * 0.74;
            let count = (w * 2.0 / 0.14).round() as usize;
            for k in 0..=count {
                let at = -w + w * 2.0 * k as f32 / count as f32;
                for (x, z) in [(at, -w), (at, w), (-w, at), (w, at)] {
                    if x == w && z.abs() < 0.22 {
                        continue;
                    }
                    m.column(v(x, f, z), 0.06, 0.055, 0.34, 6, WOOD, Finish::Matte);
                    m.column(v(x, f + 0.34, z), 0.055, 0.0, 0.1, 6, PLANK, Finish::Matte);
                }
            }
            for (x, z, sx, sz) in [(0.0, -w, w * 2.0, 0.16), (0.0, w, w * 2.0, 0.16), (-w, 0.0, 0.16, w * 2.0)] {
                m.block(v(x, f, z), v(sx, 0.08, sz), STONE);
            }
            // A watchtower of timber on four legs, with a thatched roof, and the banner of the country.
            for (x, z) in [(-0.2, -0.2), (0.2, -0.2), (-0.2, 0.2), (0.2, 0.2)] {
                m.rod(v(x - 0.15, f, z), v(x * 0.8 - 0.15, f + 0.7, z * 0.8), 0.03, WOOD_DARK);
            }
            m.block(v(-0.15, f + 0.7, 0.0), v(0.5, 0.05, 0.5), PLANK);
            for (x, z) in [(-0.22, -0.22), (0.22, -0.22), (-0.22, 0.22), (0.22, 0.22)] {
                m.rod(v(x - 0.15, f + 0.75, z), v(x - 0.15, f + 1.02, z), 0.018, WOOD);
            }
            m.column(v(-0.15, f + 1.02, 0.0), 0.4, 0.0, 0.3, 4, THATCH, Finish::Matte);
            flag(m, v(0.42, f, -0.45), 1.36, country, time, true, 0.9);
        }
        FortSize::Medium => {
            // Stone walls with a square tower at each corner, and a square keep in the middle.
            let w = half * 0.74;
            walls_round(m, f, w, 0.42, 0.22, STONE);
            for (x, z) in [(-w, -w), (w, -w), (-w, w), (w, w)] {
                square_tower(m, v(x, f, z), 0.36, 0.62, STONE_DARK);
                m.cuboid(v(x, f + 0.69, z), v(0.14, 0.02, 0.14), country, Finish::Gloss);
            }
            square_tower(m, v(-0.2, f, 0.0), 0.72, 0.85, STONE_LIGHT);
            m.cuboid(v(0.17, f + 0.18, 0.0), v(0.03, 0.34, 0.2), WOOD_DARK, Finish::Matte);
            flag(m, v(-0.2, f + 0.85, 0.0), 0.75, country, time, true, 1.0);
        }
        FortSize::Large => {
            // A low outer wall with a bastion jutting from each corner, a higher inner wall with round towers, and a tall keep
            // with a turret at each of its corners.
            let outer = half * 0.8;
            walls_round(m, f, outer, 0.3, 0.24, STONE_DARK);
            for (x, z) in [(-outer, -outer), (outer, -outer), (-outer, outer), (outer, outer)] {
                m.frustum(v(x, f, z), v(x, f + 0.4, z), 0.34, 0.28, 4, 0.0, STONE, Finish::Matte);
                m.column(v(x, f + 0.4, z), 0.12, 0.12, 0.05, 4, country, Finish::Gloss);
            }
            let inner = half * 0.5;
            walls_round(m, f, inner, 0.55, 0.2, STONE);
            for (x, z) in [(-inner, -inner), (inner, -inner), (-inner, inner), (inner, inner)] {
                m.column(v(x, f, z), 0.2, 0.18, 0.8, 12, STONE, Finish::Matte);
                m.column(v(x, f + 0.8, z), 0.23, 0.0, 0.36, 12, country, Finish::Gloss);
            }
            square_tower(m, v(0.0, f, 0.0), 0.8, 1.15, STONE_LIGHT);
            for (x, z) in [(-0.4, -0.4), (0.4, -0.4), (-0.4, 0.4), (0.4, 0.4)] {
                m.column(v(x, f + 0.8, z), 0.1, 0.1, 0.55, 10, STONE, Finish::Matte);
                m.column(v(x, f + 1.35, z), 0.13, 0.0, 0.26, 10, ROOF_SLATE, Finish::Gloss);
            }
            flag(m, v(0.0, f + 1.15, 0.0), 0.8, country, time, true, 1.2);
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// Stables.

fn stable(m: &mut Model, country: Color, time: f32) {
    let half = Look::Stable.reach();
    let f = plot(m, half, DIRT, country);
    // The long barn at the back, with a red roof, a hayloft door and the doors of the stalls on its front.
    house(m, v(-0.72, f, 0.0), v(0.55, 0.42, 1.5), FRAC_PI_2, WOOD, ROOF_RED, 0.34);
    m.cuboid(v(-0.43, f + 0.55, 0.0), v(0.02, 0.2, 0.24), WOOD_DARK, Finish::Matte);
    for z in [-0.5, -0.17, 0.17, 0.5] {
        m.cuboid(v(-0.435, f + 0.17, z), v(0.02, 0.3, 0.2), WOOD_DARK, Finish::Matte);
    }
    // The paddock in front: a fence of posts and two rails round it, open at the barn, and a horse in it.
    let (x0, x1, z0) = (-0.3, 1.0, 0.85);
    for (a, b) in [(v(x0, 0.0, -z0), v(x1, 0.0, -z0)), (v(x0, 0.0, z0), v(x1, 0.0, z0)), (v(x1, 0.0, -z0), v(x1, 0.0, z0))] {
        let steps = (a.distance(b) / 0.3).ceil() as usize;
        for k in 0..=steps {
            let p = a.lerp(b, k as f32 / steps as f32);
            m.rod(v(p.x, f, p.z), v(p.x, f + 0.2, p.z), 0.018, WOOD);
        }
        for height in [0.09, 0.17] {
            m.rod(v(a.x, f + height, a.z), v(b.x, f + height, b.z), 0.012, WOOD);
        }
    }
    horse(m, v(0.35, f, 0.2), 0.5, HORSE_COATS[0], country);
    horse(m, v(0.6, f, -0.4), -0.6, HORSE_COATS[2], country);
    // A hay bale and a trough, and the banner of the country.
    m.block(v(-0.05, f, -0.65), v(0.22, 0.2, 0.34), THATCH);
    m.block(v(0.8, f, 0.62), v(0.4, 0.1, 0.14), WOOD_DARK);
    flag(m, v(-0.5, f + 0.5, -0.9), 0.9, country, time, true, 1.0);
}

// ---------------------------------------------------------------------------------------------------------------------------
// Archery ranges.

fn archery_range(m: &mut Model, country: Color, time: f32) {
    let half = Look::ArcheryRange.reach();
    let f = plot(m, half, GRASS, country);
    // The shed at the back, with a thatched roof, open to the range, and a rack of bows.
    house(m, v(-0.72, f, 0.0), v(0.42, 0.34, 1.1), FRAC_PI_2, WOOD, THATCH, 0.34);
    for z in [-0.12, 0.0, 0.12] {
        m.rod(v(-0.44, f + 0.1, z), v(-0.44, f + 0.34, z + 0.05), 0.012, WOOD_DARK);
    }
    // The low rail to shoot from.
    for z in [-0.8, -0.4, 0.0, 0.4, 0.8] {
        m.rod(v(-0.2, f, z), v(-0.2, f + 0.16, z), 0.016, WOOD);
    }
    m.rod(v(-0.2, f + 0.15, -0.8), v(-0.2, f + 0.15, 0.8), 0.014, WOOD);
    // The targets down the range, each a round board of rings on a post, in front of a hay bale, with an arrow or two in it.
    for (k, z) in [-0.58, 0.0, 0.58].into_iter().enumerate() {
        m.block(v(0.98, f, z), v(0.22, 0.2, 0.42), THATCH);
        m.rod(v(0.84, f, z), v(0.84, f + 0.32, z), 0.022, WOOD_DARK);
        m.ball(v(0.84, f + 0.44, z), v(0.03, 0.2, 0.2), 14, CLOTH, Finish::Matte);
        m.ball(v(0.823, f + 0.44, z), v(0.03, 0.14, 0.14), 14, ROOF_RED, Finish::Matte);
        m.ball(v(0.806, f + 0.44, z), v(0.03, 0.08, 0.08), 12, CLOTH, Finish::Matte);
        m.ball(v(0.789, f + 0.44, z), v(0.03, 0.04, 0.04), 10, GOLD, Finish::Metal);
        let aim = if k == 1 { 0.05 } else { -0.06 };
        m.rod(v(0.55, f + 0.5 + aim, z + 0.02), v(0.82, f + 0.44 + aim * 0.4, z + 0.02), 0.008, WOOD);
    }
    // The banner of the country over the shed.
    flag(m, v(-0.5, f + 0.2, -0.7), 1.15, country, time, true, 1.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    const COUNTRY: Color = Color::new(0.2, 0.4, 0.9, 1.0);
    const ALL: [Look; 26] = [
        Look::Militia { figures: 1, trained: false },
        Look::Militia { figures: 5, trained: false },
        Look::Militia { figures: 3, trained: true },
        Look::Archers { figures: 1 },
        Look::Archers { figures: 5 },
        Look::Cavalry { figures: 1 },
        Look::Cavalry { figures: 3 },
        Look::Person,
        Look::Ship { large: false },
        Look::Ship { large: true },
        Look::Woodcutter,
        Look::Sawmill,
        Look::Stonecutter,
        Look::Quarry,
        Look::IronMine,
        Look::CopperMine,
        Look::SulphurPit,
        Look::OilWell,
        Look::Shipyard { large: false },
        Look::Shipyard { large: true },
        Look::Barracks,
        Look::ArcheryRange,
        Look::Stable,
        Look::Fort { size: FortSize::Small },
        Look::Fort { size: FortSize::Medium },
        Look::Fort { size: FortSize::Large },
    ];

    fn built(look: Look) -> Model {
        built_at(look, Moment { time: 1.3, busy: true })
    }

    fn built_at(look: Look, moment: Moment) -> Model {
        let mut m = Model::new(Vec3::ZERO, 1.0, 0.0);
        build(&mut m, look, COUNTRY, moment);
        m
    }

    #[test]
    fn everything_fits_in_the_room_it_says_it_takes() {
        // (Smoke and steam, while a gatherer is working, go up above it, and don't count.)
        let mut wrong = Vec::new();
        for look in ALL {
            let m = built_at(look, Moment { time: 1.3, busy: false });
            assert!(m.len() > 100 && m.len() < 12_000, "{look:?}: {} corners, which is too many to build every frame", m.len());
            let (low, high) = m.bounds();
            let reach = look.reach();
            assert!(low.x > -reach * 1.25 && high.x < reach * 1.25 && low.z > -reach * 1.25 && high.z < reach * 1.25, "{look:?}: {low} {high} for a reach of {reach}");
            if !(high.y <= look.height() + 0.02 && high.y > look.height() * 0.8) {
                wrong.push(format!("{look:?}: {} high, said to be {}", high.y, look.height()));
            }
            assert!(low.y > -0.3, "{look:?}: only a ship's keel goes below the ground, and not far: {}", low.y);
        }
        assert!(wrong.is_empty(), "{}", wrong.join("
"));
    }

    #[test]
    fn the_simple_forms_are_simpler_and_stand_in_the_same_room() {
        let mut wrong = Vec::new();
        for look in ALL {
            let mut m = Model::new(Vec3::ZERO, 1.0, 0.0);
            build_simple(&mut m, look, COUNTRY);
            let full = built_at(look, Moment { time: 1.3, busy: false });
            assert!(m.len() > 20 && m.len() * 2 < full.len(), "{look:?}: {} corners, against {} in full", m.len(), full.len());
            let (low, high) = m.bounds();
            let reach = look.reach();
            assert!(low.x > -reach * 1.25 && high.x < reach * 1.25 && low.z > -reach * 1.25 && high.z < reach * 1.25, "{look:?}: {low} {high} for a reach of {reach}");
            if !(high.y <= look.height() + 0.02 && high.y > look.height() * 0.5) {
                wrong.push(format!("{look:?}: {} high, said to be {}", high.y, look.height()));
            }
            assert!(low.y > -0.3, "{look:?}: {}", low.y);
        }
        assert!(wrong.is_empty(), "{}", wrong.join("
"));
    }

    #[test]
    fn more_members_are_more_figures_and_soldiers_are_not_militia() {
        let count = |figures, trained| built(Look::Militia { figures, trained }).len();
        assert!(count(1, false) < count(2, false) && count(2, false) < count(5, false));
        assert_ne!(count(3, false), count(3, true));
        assert_eq!(formation(9).len(), 5);
    }

    #[test]
    fn cavalry_are_horsemen_that_are_bigger_than_a_band_on_foot() {
        let count = |figures| built(Look::Cavalry { figures }).len();
        assert!(count(1) < count(2) && count(2) < count(3));
        assert!(count(3) > built(Look::Militia { figures: 3, trained: true }).len(), "a horse is more to build than a man");
        assert!(Look::Cavalry { figures: 3 }.height() > Look::Archers { figures: 3 }.height());
        assert!(Look::Stable.reach() > Look::Barracks.reach() * 0.9 && Look::Stable.reach() > Look::ArcheryRange.reach());
    }

    #[test]
    fn archers_are_a_band_of_their_own_with_more_figures_for_more_members() {
        let count = |figures| built(Look::Archers { figures }).len();
        assert!(count(1) < count(3) && count(3) < count(5));
        assert_ne!(count(3), built(Look::Militia { figures: 3, trained: true }).len());
        assert_ne!(count(3), built(Look::Militia { figures: 3, trained: false }).len());
        assert!(Look::ArcheryRange.reach() < Look::Barracks.reach(), "a smaller building than a barracks");
    }

    #[test]
    fn a_large_one_is_bigger_than_a_small_one() {
        assert!(Look::Ship { large: true }.reach() > Look::Ship { large: false }.reach());
        assert!(Look::Shipyard { large: true }.reach() > Look::Shipyard { large: false }.reach());
        assert!(Look::Sawmill.reach() > Look::Woodcutter.reach() && Look::Quarry.reach() > Look::Stonecutter.reach());
        assert!(built(Look::Ship { large: true }).len() > built(Look::Ship { large: false }).len());
    }

    #[test]
    fn what_is_working_moves_and_what_is_not_stands_still() {
        let at = |look, time, busy| {
            let mut m = Model::new(Vec3::ZERO, 1.0, 0.0);
            build(&mut m, look, COUNTRY, Moment { time, busy });
            crate::model::tests::sum(&m)
        };
        assert_ne!(at(Look::OilWell, 0.4, true), at(Look::OilWell, 1.4, true), "the pump nods");
        assert_eq!(at(Look::OilWell, 0.4, false), at(Look::OilWell, 1.4, false), "and not while the well isn't working");
    }
}
