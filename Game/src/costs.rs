//! What each thing the player can build costs and what it does for them, in words, for the footer buttons to say when
//! the mouse is on them and for the prompts to say while a tool is on. The figures come from where the game keeps them,
//! so these can't drift from what happens.

use macroquad::prelude::*;
use nworld_view::style::{self, Style, Weight};

use crate::bridge::BRIDGE_COST;
use crate::failure::{BRIDGE_FAILURE, PATH_FAILURE, ROAD_FAILURE};
use crate::format;
use crate::gatherer::GathererKind;
use crate::hud;
use crate::path::{Kind, PATH_COST, ROAD_COST};

/// A thing that can be built, and so has a cost.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Buildable {
    /// A path or a road.
    Way(Kind),
    Bridge,
    Gatherer(GathererKind),
}

impl Buildable {
    /// What it is called, to begin a sentence: "Path", "Road", "Bridge", "Woodcutter" or "Stonecutter".
    pub fn label(self) -> &'static str {
        match self {
            Self::Way(Kind::Path) => "Path",
            Self::Way(Kind::Road) => "Road",
            Self::Bridge => "Bridge",
            Self::Gatherer(kind) => kind.label(),
        }
    }

    /// What it costs, in coins: for a path or a road, a segment of it.
    pub fn cost(self) -> i64 {
        match self {
            Self::Way(Kind::Path) => PATH_COST,
            Self::Way(Kind::Road) => ROAD_COST,
            Self::Bridge => BRIDGE_COST,
            Self::Gatherer(kind) => kind.cost(),
        }
    }

    /// What it costs, as it is said: "10 coins a segment" or "1,000 coins".
    pub fn cost_text(self) -> String {
        let coins = format::money(self.cost());
        match self {
            Self::Way(_) => format!("{coins} coins a segment"),
            _ => format!("{coins} coins"),
        }
    }

    /// What it does for the player, in a sentence or two. A path, a road or a bridge makes nothing of its own: it
    /// joins things up, and can fail.
    pub fn output(self) -> String {
        match self {
            Self::Way(Kind::Path) => format!("Joins regions into routes. Fails {} of days.", percent(PATH_FAILURE)),
            Self::Way(Kind::Road) => format!("Joins regions into routes. Fails {} of days.", percent(ROAD_FAILURE)),
            Self::Bridge => format!("Carries a route across a river. Fails {} of days.", percent(BRIDGE_FAILURE)),
            Self::Gatherer(kind) => format!("Provides {} {} a day while it has a route to a city.", kind.daily_yield(), kind.product()),
        }
    }

    /// The lines a button says when the mouse is on it: what it is and costs, and what it does, and for a failed one,
    /// that building it again mends it for the same.
    pub fn button_lines(self) -> Vec<(String, Color)> {
        let mut lines = vec![(format!("{}: {}.", self.label(), self.cost_text()), style::TEXT), (self.output(), style::TEXT_MUTED)];
        if !matches!(self, Self::Gatherer(_)) {
            lines.push(("A failed one is mended by building it again, for the same.".to_owned(), style::TEXT_MUTED));
        }
        lines
    }
}

/// A chance, as a fraction, as a percent in the fewest figures that say it: 0.02 as "2%" and 0.0005 as "0.05%".
pub fn percent(chance: f32) -> String {
    let text = format!("{:.2}", chance * 100.0);
    format!("{}%", text.trim_end_matches('0').trim_end_matches('.'))
}

/// Draws a card of `lines` above `button`, with its right edge at the button's, for when the mouse is on it. Each line
/// is wrapped to the card.
pub fn draw_tip(style: &Style, button: Rect, lines: &[(String, Color)]) {
    let wrapped: Vec<(String, Color)> = lines.iter().flat_map(|(text, color)| hud::wrap(style, text, 12, 260.0).into_iter().map(move |line| (line, *color))).collect();
    let height = wrapped.len() as f32 * 17.0 + 20.0;
    let tip = Rect::new((button.right() - 300.0).max(8.0), button.y - 8.0 - height, 300.0, height);
    style::card(tip, 10.0, style::GLASS);
    for (k, (line, color)) in wrapped.iter().enumerate() {
        style.text_mid(line, tip.x + 14.0, tip.y + 10.0 + 17.0 * (k as f32 + 0.5), 12, Weight::Regular, *color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chance_is_a_percent_in_the_fewest_figures() {
        assert_eq!(percent(PATH_FAILURE), "2%");
        assert_eq!(percent(ROAD_FAILURE), "0.1%");
        assert_eq!(percent(BRIDGE_FAILURE), "0.05%");
        assert_eq!(percent(0.5), "50%");
        assert_eq!(percent(0.0), "0%");
    }

    #[test]
    fn what_each_thing_costs_is_what_the_game_charges() {
        assert_eq!(Buildable::Way(Kind::Path).cost(), PATH_COST);
        assert_eq!(Buildable::Way(Kind::Road).cost(), ROAD_COST);
        assert_eq!(Buildable::Bridge.cost(), BRIDGE_COST);
        for kind in GathererKind::ALL {
            assert_eq!(Buildable::Gatherer(kind).cost(), kind.cost());
        }
        assert_eq!(Buildable::Way(Kind::Path).cost_text(), "10 coins a segment");
        assert_eq!(Buildable::Way(Kind::Road).cost_text(), "100 coins a segment");
        assert_eq!(Buildable::Bridge.cost_text(), "1,000 coins");
        assert_eq!(Buildable::Gatherer(GathererKind::Stonecutter).cost_text(), "1,000 coins");
    }

    #[test]
    fn what_each_thing_does_is_said_with_the_games_own_figures() {
        assert_eq!(Buildable::Way(Kind::Path).output(), "Joins regions into routes. Fails 2% of days.");
        assert_eq!(Buildable::Way(Kind::Road).output(), "Joins regions into routes. Fails 0.1% of days.");
        assert_eq!(Buildable::Bridge.output(), "Carries a route across a river. Fails 0.05% of days.");
        assert_eq!(Buildable::Gatherer(GathererKind::Woodcutter).output(), "Provides 100 wood a day while it has a route to a city.");
        assert_eq!(Buildable::Gatherer(GathererKind::Stonecutter).output(), "Provides 100 stone a day while it has a route to a city.");
    }

    #[test]
    fn a_button_says_what_it_is_costs_and_does_and_only_what_can_fail_says_it_is_mended() {
        let bridge: Vec<String> = Buildable::Bridge.button_lines().into_iter().map(|(text, _)| text).collect();
        assert_eq!(bridge[0], "Bridge: 1,000 coins.");
        assert_eq!(bridge.len(), 3);
        assert!(bridge[2].contains("mended"));
        let stone: Vec<String> = Buildable::Gatherer(GathererKind::Stonecutter).button_lines().into_iter().map(|(text, _)| text).collect();
        assert_eq!(stone, ["Stonecutter: 1,000 coins.", "Provides 100 stone a day while it has a route to a city."]);
    }
}
