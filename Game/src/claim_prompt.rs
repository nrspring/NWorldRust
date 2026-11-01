//! The question asked after something is placed in a region that no country has: a path, a road, a bridge or a
//! gatherer. Should the region be claimed for the country being played? A region that was turned down is not asked about
//! again until the game is next run.

use std::collections::HashSet;

use macroquad::prelude::*;
use nworld_view::style::{self, Style, Weight};

use crate::day::DaySave;
use crate::format::capitalized;
use crate::popup::{self, Icon};

const CARD_WIDTH: f32 = 460.0;
const PADDING: f32 = 24.0;
const BUTTON_HEIGHT: f32 = 40.0;
/// How long the popup takes to appear, in seconds.
const FADE_IN: f64 = 0.22;

#[derive(Default)]
pub struct ClaimPrompt {
    /// The region being asked about.
    asking: Option<u16>,
    /// The regions to ask about after it.
    waiting: Vec<u16>,
    /// The regions the player said no to.
    declined: HashSet<u16>,
    /// When the question came up, once it is first drawn.
    opened_at: Option<f64>,
    /// Set when the question has just come up, so the click that placed the thing isn't also an answer.
    fresh: bool,
    /// Why the last try at claiming couldn't be done.
    error: Option<String>,
}

impl ClaimPrompt {
    pub fn is_open(&self) -> bool {
        self.asking.is_some()
    }

    /// Notes that something was placed in each of `regions`: those that no country has, and that weren't turned down
    /// before, are asked about.
    pub fn placed_in(&mut self, day: &DaySave, regions: &[u16]) {
        if day.player_country().is_none() {
            return;
        }
        for &region in regions {
            if day.region_owner(region).is_none() && !self.declined.contains(&region) && self.asking != Some(region) && !self.waiting.contains(&region) {
                self.waiting.push(region);
            }
        }
        self.next();
    }

    /// What Esc does: says no to the region being asked about.
    pub fn escape(&mut self) {
        self.decline();
    }

    /// Shows why the region couldn't be claimed.
    pub fn fail(&mut self, message: String) {
        self.error = Some(message);
    }

    /// Carries on to the next region after one was claimed.
    pub fn claimed(&mut self) {
        self.asking = None;
        self.next();
    }

    fn decline(&mut self) {
        if let Some(region) = self.asking.take() {
            self.declined.insert(region);
        }
        self.next();
    }

    /// Goes on to ask about the next region waiting, if none is being asked about.
    fn next(&mut self) {
        if self.asking.is_none() && !self.waiting.is_empty() {
            self.asking = Some(self.waiting.remove(0));
            self.opened_at = None;
            self.fresh = true;
            self.error = None;
        }
    }

    /// Takes the mouse for this frame and draws the question over everything. Returns the region when the player says
    /// to claim it, for the caller to do.
    pub fn show(&mut self, style: &Style, day: &DaySave) -> Option<u16> {
        let region = self.asking?;
        // Something else may have claimed it, or the country may be gone, since it was asked.
        let Some(country) = day.player_country().filter(|_| day.region_owner(region).is_none()) else {
            self.asking = None;
            self.next();
            return None;
        };
        let mouse = Vec2::from(mouse_position());
        let clicked = is_mouse_button_pressed(MouseButton::Left) && !std::mem::take(&mut self.fresh);

        let opened_at = *self.opened_at.get_or_insert_with(get_time);
        let t = (((get_time() - opened_at) / FADE_IN).clamp(0.0, 1.0)) as f32;
        let ease = 1.0 - (1.0 - t) * (1.0 - t);
        let (screen_w, screen_h) = (screen_width(), screen_height());
        draw_rectangle(0.0, 0.0, screen_w, screen_h, Color::new(0.02, 0.03, 0.05, 0.6 * ease));

        let inner_w = CARD_WIDTH - PADDING * 2.0;
        let height = 112.0 + BUTTON_HEIGHT + PADDING;
        let card = Rect::new((screen_w - CARD_WIDTH) * 0.5, (screen_h - height) * 0.5 + (1.0 - ease) * 18.0, CARD_WIDTH, height);
        style::card(card, 18.0, style::PANEL);
        let inner_x = card.x + PADDING;

        let badge = Rect::new(card.x + PADDING, card.y + PADDING, 40.0, 40.0);
        style::rounded_rect(badge, 10.0, style::ACCENT_SOFT);
        popup::draw_icon(Icon::Region, badge);
        let text_x = badge.right() + 14.0;
        style.text("Claim this region?", text_x, card.y + PADDING + 17.0, 20, Weight::Semibold, style::TEXT);
        let label = day.region_label(region);
        style.text(&format!("{} has no country.", capitalized(&label)), text_x, card.y + PADDING + 36.0, 12, Weight::Regular, style::TEXT_MUTED);
        style.text(&format!("Should it belong to {}?", country.name), inner_x, card.y + 88.0, 14, Weight::Regular, style::TEXT);
        if let Some(error) = &self.error {
            style.text(error, inner_x, card.y + 108.0, 12, Weight::Regular, style::ERROR);
        }

        // Not now and Claim, side by side.
        let top = card.bottom() - PADDING - BUTTON_HEIGHT;
        let decline = Rect::new(inner_x, top, (inner_w - 12.0) * 0.5, BUTTON_HEIGHT);
        let claim = Rect::new(decline.right() + 12.0, top, decline.w, BUTTON_HEIGHT);
        let (over_decline, over_claim) = (decline.contains(mouse), claim.contains(mouse));
        style::rounded_rect(decline, 8.0, if over_decline { style::BORDER_STRONG } else { style::HOVER });
        style.text_centered("Not now", decline, 14, Weight::Semibold, style::TEXT);
        style::rounded_rect(claim, 8.0, if over_claim { style::ACCENT_HOVER } else { style::ACCENT });
        style.text_centered("Claim it", claim, 14, Weight::Semibold, WHITE);

        // Only a click on Claim goes ahead: no key does it.
        if clicked && over_claim {
            return Some(region);
        }
        if clicked && (over_decline || !card.contains(mouse)) {
            self.decline();
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::Date;

    fn day() -> DaySave {
        let mut day = DaySave::new(Date::FIRST, "x");
        day.found_country("Aldoria").unwrap();
        day.found_country("Brevon").unwrap();
        day
    }

    #[test]
    fn a_region_nobody_has_is_asked_about_once_and_each_of_several_in_turn() {
        let day = day();
        let mut prompt = ClaimPrompt::default();
        assert!(!prompt.is_open());
        prompt.placed_in(&day, &[0x4005, 0x4005, 0x4006]);
        assert_eq!((prompt.asking, prompt.waiting.clone()), (Some(0x4005), vec![0x4006]));
        // Saying no goes on to the next, and neither is asked about again.
        prompt.escape();
        assert_eq!(prompt.asking, Some(0x4006));
        prompt.escape();
        assert!(!prompt.is_open());
        prompt.placed_in(&day, &[0x4005, 0x4006]);
        assert!(!prompt.is_open(), "they were turned down");
        prompt.placed_in(&day, &[0x4007]);
        assert_eq!(prompt.asking, Some(0x4007));
        prompt.claimed();
        assert!(!prompt.is_open());
    }

    #[test]
    fn a_region_that_has_a_country_is_not_asked_about_and_nor_is_a_game_with_no_country() {
        let mut day = day();
        day.claim_region(0x4005).unwrap();
        let mut prompt = ClaimPrompt::default();
        prompt.placed_in(&day, &[0x4005]);
        assert!(!prompt.is_open());
        prompt.placed_in(&DaySave::new(Date::FIRST, "x"), &[0x4006]);
        assert!(!prompt.is_open());
    }

    #[test]
    fn region_names_start_with_a_capital() {
        assert_eq!(capitalized("region 5"), "Region 5");
        assert_eq!(capitalized("The Marches"), "The Marches");
        assert_eq!(capitalized(""), "");
    }
}
