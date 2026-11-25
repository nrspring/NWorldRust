//! Showing where the militia that was clicked can go: every cell of ground it can still get to is tinted over the map,
//! green where it would have most of its movement points left and amber where it would have the least, and a panel says
//! what it is and how many points it has. This is the game's own drawing, and not part of drawing the map.

use macroquad::models::{draw_mesh, Mesh};
use macroquad::prelude::*;
use nworld_map::world::World;
use nworld_view::style::{self, Style, Weight};

use crate::calendar::Date;
use crate::day::DaySave;
use crate::hud;
use crate::failure::Thing;
use crate::ids::{Guid, MilitiaId, ShipId};
use crate::movement::{self, Network, Reach};
use crate::movement_mesh;
use crate::path::{Kind, PathPiece};
use crate::path_view::PathRenderer;

const PANEL_SIZE: Vec2 = Vec2::new(500.0, 70.0);
const PANEL_MARGIN: f32 = 12.0;

/// What changed in the militia panel.
#[derive(Debug, PartialEq, Eq)]
pub enum PanelAction {
    /// The player asked for the militia's popup.
    Details,
    /// The player put the militia away.
    Deselect,
}

/// What the overlay was made for, to know when to make it again.
#[derive(Clone, Debug, PartialEq)]
struct Key {
    id: Guid,
    at: (u32, u32),
    points: u32,
    date: Date,
    /// How many paths, roads, bridges, links and failures there were, which is what changes where it can go.
    counts: [usize; 6],
}

/// The overlay for the militia that is selected, made again when it, or the ground it goes over, changes.
#[derive(Default)]
pub struct MovementView {
    key: Option<Key>,
    reach: Option<Reach>,
    meshes: Vec<Mesh>,
}

impl MovementView {
    /// Makes sure the overlay is for `selected`, which is nothing at all if it is `None` or not in `day`. The renderers
    /// must have drawn the paths and roads of `day`, which is where they are found.
    pub fn update(&mut self, world: &World, day: &DaySave, paths: &PathRenderer, roads: &PathRenderer, selected: Option<MilitiaId>) {
        let Some(militia) = selected.and_then(|id| day.militia(id)) else {
            *self = Self::default();
            return;
        };
        let counts = [day.paths.len(), day.roads.len(), day.bridges.len(), day.bridge_links.len() + day.road_links.len(), day.failed.len(), 0];
        let key = Key { id: militia.id.into(), at: (militia.x.to_bits(), militia.z.to_bits()), points: militia.points, date: day.date, counts };
        if self.key.as_ref() == Some(&key) {
            return;
        }
        let reach = movement::reach(world, &network_of(day, paths, roads), vec2(militia.x, militia.z), militia.points as f32);
        self.meshes = movement_mesh::build(world, &reach);
        self.reach = Some(reach);
        self.key = Some(key);
    }

    /// Makes sure the overlay is for the ship `selected`, which is nothing at all if it is `None` or not in `day`. A ship can get to the
    /// water its size sails, and nowhere else, so the paths and roads are nothing to it.
    pub fn update_ship(&mut self, world: &World, day: &DaySave, selected: Option<ShipId>) {
        let Some(ship) = selected.and_then(|id| day.ship(id)) else {
            *self = Self::default();
            return;
        };
        let key = Key { id: ship.id.into(), at: (ship.x.to_bits(), ship.z.to_bits()), points: ship.points, date: day.date, counts: [0; 6] };
        if self.key.as_ref() == Some(&key) {
            return;
        }
        let kind = ship.kind;
        let reach = movement::reach_by_water(world, &|water| kind.can_sail(water), kind.water_cost(), vec2(ship.x, ship.z), ship.points as f32);
        self.meshes = movement_mesh::build(world, &reach);
        self.reach = Some(reach);
        self.key = Some(key);
    }

    /// Where the selected militia or ship can get to, if one is selected.
    pub fn reach(&self) -> Option<&Reach> {
        self.reach.as_ref()
    }

    /// Draws the overlay, as seen by the camera that is set, which should be after everything else on the map.
    pub fn draw(&self) {
        for mesh in &self.meshes {
            draw_mesh(mesh);
        }
    }
}

/// The paths, roads and bridges of `day` that stand, along the lines the renderers have drawn them on.
fn network_of(day: &DaySave, paths: &PathRenderer, roads: &PathRenderer) -> Network {
    let lines = |kind: Kind, renderer: &PathRenderer| -> Vec<Vec<Vec2>> {
        let pieces = day.segments(kind).iter().map(|s| PathPiece::Segment(*s)).chain(day.links(kind).iter().map(|l| PathPiece::Link(*l)));
        pieces.filter(|piece| !day.is_failed(Thing::of(kind, *piece))).flat_map(|piece| renderer.route_of(piece).to_vec()).collect()
    };
    Network {
        paths: lines(Kind::Path, paths),
        roads: lines(Kind::Road, roads),
        bridges: day.bridges.iter().filter(|b| !day.is_failed(Thing::Bridge(b.id))).map(|b| (b.first.into(), b.second.into())).collect(),
    }
}

/// Where the militia panel is: in the middle, above the footer. The mouse over it is not over the map.
pub fn panel_rect() -> Rect {
    Rect::new((screen_width() - PANEL_SIZE.x) * 0.5, screen_height() - hud::FOOTER_HEIGHT - PANEL_MARGIN - PANEL_SIZE.y, PANEL_SIZE.x, PANEL_SIZE.y)
}

/// Draws the panel for the selected militia or ship called `title`, which has `points` movement points left of the `full` it gets each
/// day, and says what the player clicked on it, if anything.
pub fn panel(style: &Style, title: &str, points: u32, full: u32, error: Option<&str>) -> Option<PanelAction> {
    let panel = panel_rect();
    let mouse = Vec2::from(mouse_position());
    style::card(panel, 12.0, style::GLASS);
    style.text_mid(title, panel.x + 18.0, panel.y + 22.0, 16, Weight::Semibold, style::TEXT);
    let status = format!("{points} of {full} movement points left");
    let color = if points == 0 { style::ERROR } else { style::TEXT_MUTED };
    style.text_mid(&status, panel.x + 18.0, panel.y + 46.0, 13, Weight::Regular, color);
    let hint = match (error, points) {
        (Some(error), _) => error,
        (None, 0) => "It can't move again today",
        _ => "Click a tinted spot to move there",
    };
    let hint_x = panel.x + 18.0 + style.measure(&status, 13, Weight::Regular).width + 14.0;
    style.text_mid(hint, hint_x, panel.y + 46.0, 13, Weight::Regular, if error.is_some() { style::ERROR } else { style::TEXT_MUTED });

    let mut action = None;
    let clicked = is_mouse_button_pressed(MouseButton::Left);
    for (n, (label, what)) in [("Details", PanelAction::Details), ("Close", PanelAction::Deselect)].into_iter().enumerate() {
        let button = Rect::new(panel.right() - 18.0 - 74.0 * (2 - n) as f32 - 8.0 * (1 - n) as f32, panel.y + 20.0, 74.0, 30.0);
        let over = button.contains(mouse);
        style::rounded_rect(button, 8.0, if over { style::BORDER_STRONG } else { style::HOVER });
        style.text_centered(label, button, 13, Weight::Semibold, style::TEXT);
        if over && clicked {
            action = Some(what);
        }
    }
    action
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_is_shown_until_a_militia_is_selected() {
        let view = MovementView::default();
        assert!(view.reach().is_none() && view.meshes.is_empty());
    }
}
