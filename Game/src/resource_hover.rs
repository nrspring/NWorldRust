//! Which resource lies under a point of the map, for telling the player what a deposit is when the mouse is over it.

use std::f32::consts::PI;

use nworld_map::world::{Resource, World};

/// How much of the ground around a point must be covered by a resource for it to count as there. A deposit's painted
/// edge fades out, and this keeps the faint last of it from being reported.
const LEAST_COVER: f32 = 0.04;

/// A resource found under the mouse, and how thickly it covers the ground there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Deposit {
    pub resource: Resource,
    /// From 0 (none) to 1 (fully covered), over the area looked at.
    pub cover: f32,
}

impl Deposit {
    /// How rich the deposit is, in a word.
    pub fn richness(&self) -> &'static str {
        match self.cover {
            c if c < 0.35 => "Sparse",
            c if c < 0.7 => "Moderate",
            _ => "Dense",
        }
    }
}

/// The resource that covers most of the ground within `reach` meters of (`x`, `z`), if any covers enough to count.
pub fn deposit_near(world: &World, x: f32, z: f32, reach: f32) -> Option<Deposit> {
    let area = PI * reach * reach;
    Resource::ALL
        .into_iter()
        .map(|resource| Deposit { resource, cover: (world.resource_near(resource, x, z, reach) / area).min(1.0) })
        .filter(|deposit| deposit.cover >= LEAST_COVER)
        .max_by(|a, b| a.cover.total_cmp(&b.cover))
}

#[cfg(test)]
mod tests {
    use super::*;
    use nworld_map::world::{CELL_SIZE, SEA_LEVEL};

    /// A world with a patch of `resource` at `density` in samples 44 to 83 each way.
    fn world_with(resource: Resource, density: u8) -> World {
        let mut world = World::new(512.0, 512.0);
        let (nx, nz) = world.samples();
        world.set_terrain(vec![10.0; nx * nz], vec![SEA_LEVEL; nx * nz]);
        for j in 44..84 {
            for i in 44..84 {
                world.resource_mut(resource)[j * nx + i] = density;
            }
        }
        world
    }

    fn middle(world: &World) -> f32 {
        63.5 * CELL_SIZE - world.half_extents().x
    }

    #[test]
    fn ground_with_nothing_painted_has_no_deposit() {
        let world = World::new(512.0, 512.0);
        assert_eq!(deposit_near(&world, 0.0, 0.0, 6.0), None);
    }

    #[test]
    fn a_painted_patch_is_found_by_its_kind() {
        for resource in Resource::ALL {
            let world = world_with(resource, 255);
            let at = middle(&world);
            let deposit = deposit_near(&world, at, at, 6.0).unwrap();
            assert_eq!(deposit.resource, resource);
            assert!(deposit.cover > 0.5, "a small circle holds few samples, so it reads low: {}", deposit.cover);
        }
    }

    #[test]
    fn ground_beside_the_patch_has_none() {
        let world = world_with(Resource::Iron, 255);
        let away = middle(&world) + 400.0 * 0.5;
        assert_eq!(deposit_near(&world, away, middle(&world), 6.0), None);
    }

    #[test]
    fn where_two_overlap_the_denser_one_wins() {
        let mut world = world_with(Resource::Stone, 80);
        let (nx, _) = world.samples();
        for j in 44..84 {
            for i in 44..84 {
                world.resource_mut(Resource::Copper)[j * nx + i] = 220;
            }
        }
        let at = middle(&world);
        assert_eq!(deposit_near(&world, at, at, 6.0).unwrap().resource, Resource::Copper);
    }

    #[test]
    fn richness_goes_from_sparse_to_dense() {
        let at = |cover| Deposit { resource: Resource::Oil, cover }.richness();
        assert_eq!((at(0.1), at(0.5), at(0.95)), ("Sparse", "Moderate", "Dense"));
    }
}
