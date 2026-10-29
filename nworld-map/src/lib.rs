//! The NWorld map: the terrain, the water standing on it, the resources and regions laid over it,
//! the generators and brushes that build them, saving and loading, and the questions a game can ask
//! of it. Nothing here draws anything: the editor, or any other application, builds its own
//! display from [`world::World`].
//!
//! * [`world`]: the map itself, and the queries (what water is here, how much stone is nearby,
//!   which region is this).
//! * [`storage`]: the map file.
//! * [`build`]: making a map, step by step: continents, islands, mountains, lakes, rivers,
//!   shallows, passes and regions, each a method of `World`.
//! * [`continents`], [`islands`], [`hills`], [`rivers`], [`shallows`], [`passes`], [`regions`] and
//!   [`density`]: what those steps are made of, which works on the plain arrays of a world, and
//!   the brushes an editor paints with. A program that only reads maps never needs them.
//! * [`noise`] and [`gen_common`]: what the generators share.
//!
//! The editor draws maps; other applications are free to draw them their own way, and to lay
//! roads, cities and armies over them. None of that belongs here.

pub mod build;
pub mod continents;
pub mod density;
pub mod gen_common;
pub mod hills;
pub mod islands;
pub mod noise;
pub mod passes;
pub mod regions;
pub mod rivers;
pub mod shallows;
pub mod storage;
pub mod world;
