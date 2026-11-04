//! The shipyard popup, opened by clicking a shipyard: what there is to know of it, and a button to upgrade it to the next size.

use macroquad::prelude::*;
use nworld_view::style::{self, Style, Weight};

use crate::day::DaySave;
use crate::ids::ShipyardId;
use crate::newsreel::cost_text;
use crate::shipyard::upgrade_full_cost;

const CARD_WIDTH: f32 = 380.0;
const PADDING: f32 = 24.0;
const ROW_HEIGHT: f32 = 36.0;
const BUTTON_HEIGHT: f32 = 40.0;
/// Where the rows start, below the card's top: room for the title.
const ROWS_TOP: f32 = 84.0;
/// The room for upgrading: the reason it can't be done or what it costs, and the button.
const UPGRADE_HEIGHT: f32 = 12.0 + 34.0 + 6.0 + BUTTON_HEIGHT;
/// How long the popup takes to appear, in seconds.
const FADE_IN: f64 = 0.22;

/// What the player changed about the shipyard in the popup.
#[derive(Debug, PartialEq)]
pub enum Change {
    /// The shipyard is upgraded to the next size.
    Upgrade(ShipyardId),
}

impl Change {
    /// Does it to `day`. An error says why it can't be done.
    pub fn apply(&self, day: &mut DaySave) -> Result<(), String> {
        match self {
            Self::Upgrade(id) => day.upgrade_shipyard(*id),
        }
    }
}

#[derive(Default)]
pub struct ShipyardPopup {
    /// The shipyard being shown, while the popup is up.
    shipyard: Option<ShipyardId>,
    opened_at: f64,
    /// Set when the popup has just come up, so the click that did it isn't also a click on the popup.
    fresh: bool,
    /// Why the last change couldn't be made.
    error: Option<String>,
}

impl ShipyardPopup {
    pub fn is_open(&self) -> bool {
        self.shipyard.is_some()
    }

    pub fn open(&mut self, shipyard: ShipyardId) {
        *self = Self { shipyard: Some(shipyard), opened_at: get_time(), fresh: true, ..Self::default() };
    }

    /// What Esc does: closes the popup.
    pub fn escape(&mut self) {
        self.shipyard = None;
    }

    /// Shows why the change that `show` gave couldn't be made.
    pub fn fail(&mut self, message: String) {
        self.error = Some(message);
    }

    /// Takes the mouse and Enter for this frame and draws the popup over everything. A shipyard that is not in `day` is not
    /// shown. Returns what the player changed about the shipyard, if they did.
    pub fn show(&mut self, style: &Style, day: &DaySave) -> Option<Change> {
        let Some(yard) = self.shipyard.and_then(|id| day.shipyard(id)) else {
            self.shipyard = None;
            return None;
        };
        let mouse = Vec2::from(mouse_position());
        let clicked = is_mouse_button_pressed(MouseButton::Left) && !std::mem::take(&mut self.fresh);
        let mut change = None;

        let t = (((get_time() - self.opened_at) / FADE_IN).clamp(0.0, 1.0)) as f32;
        let ease = 1.0 - (1.0 - t) * (1.0 - t);
        let (screen_w, screen_h) = (screen_width(), screen_height());
        draw_rectangle(0.0, 0.0, screen_w, screen_h, Color::new(0.02, 0.03, 0.05, 0.6 * ease));

        let error_height = if self.error.is_some() { 24.0 } else { 0.0 };
        let height = ROWS_TOP + ROW_HEIGHT * 3.0 + UPGRADE_HEIGHT + 16.0 + error_height + BUTTON_HEIGHT + PADDING;
        let card = Rect::new((screen_w - CARD_WIDTH) * 0.5, (screen_h - height) * 0.5 + (1.0 - ease) * 18.0, CARD_WIDTH, height);
        style::card(card, 18.0, style::PANEL);

        // The title, beside a little ship.
        let badge = Rect::new(card.x + PADDING, card.y + PADDING, 40.0, 40.0);
        style::rounded_rect(badge, 10.0, style::ACCENT_SOFT);
        yard.kind.draw_icon(badge.center(), 1.5, style::ACCENT_HOVER, style::ACCENT_SOFT);
        let text_x = badge.right() + 14.0;
        style.text(yard.kind.label(), text_x, card.y + PADDING + 17.0, 20, Weight::Semibold, style::TEXT);
        style.text("Shipyard", text_x, card.y + PADDING + 36.0, 12, Weight::Regular, style::TEXT_MUTED);

        // What there is to know, a row each.
        let inner_x = card.x + PADDING;
        let inner_w = card.w - PADDING * 2.0;
        let rows: Vec<Rect> = (0..3).map(|k| Rect::new(inner_x, card.y + ROWS_TOP + ROW_HEIGHT * k as f32, inner_w, ROW_HEIGHT)).collect();
        for y in [rows[0].y, rows[1].y, rows[2].y, rows[2].bottom()] {
            draw_line(inner_x, y, inner_x + inner_w, y, 1.0, style::BORDER);
        }
        let country = day.country(yard.country);
        let region = yard.region.map_or_else(|| "None".to_owned(), |region| day.region_name(region).map_or_else(|| format!("Region {}", region & 0x3FFF), str::to_owned));
        let size = yard.kind.label().split(' ').next().unwrap_or_default().to_owned();
        let value_row = |label: &str, value: &str, row: Rect, swatch: Option<Color>| {
            style.text_mid(label, inner_x, row.center().y, 13, Weight::Regular, style::TEXT_MUTED);
            let x = row.right() - style.measure(value, 14, Weight::Semibold).width;
            style.text_mid(value, x, row.center().y, 14, Weight::Semibold, style::TEXT);
            if let Some(color) = swatch {
                style::rounded_rect(Rect::new(x - 18.0, row.center().y - 6.0, 12.0, 12.0), 3.0, color);
            }
        };
        value_row("Country", country.map_or("an unknown country", |c| c.name.as_str()), rows[0], country.map(|c| c.color.color()));
        value_row("Region", &region, rows[1], None);
        value_row("Size", &size, rows[2], None);

        // Upgrading it: what it costs, or why it can't be done, and the button. A large shipyard is as big as it gets.
        let problem = day.upgrade_problem(yard.id);
        let next = yard.kind.upgrade();
        let upgradable = next.is_some() && problem.is_none();
        let note_rect = Rect::new(inner_x, rows[2].bottom() + 12.0, inner_w, 34.0);
        let button = Rect::new(inner_x, note_rect.bottom() + 6.0, inner_w, BUTTON_HEIGHT);
        let note = match (&problem, next) {
            (_, None) => "It is the largest size.".to_owned(),
            (Some(problem), _) => problem.clone(),
            (None, Some(_)) => format!("Costs {}.", cost_text(&upgrade_full_cost())),
        };
        for (n, line) in crate::hud::wrap(style, &note, 12, inner_w).iter().take(2).enumerate() {
            style.text_mid(line, note_rect.x, note_rect.y + 9.0 + n as f32 * 15.0, 12, Weight::Regular, style::TEXT_MUTED);
        }
        let over_upgrade = button.contains(mouse) && upgradable;
        let fill = if !upgradable { style::HOVER } else if over_upgrade { style::ACCENT_HOVER } else { style::ACCENT };
        style::rounded_rect(button, 8.0, fill);
        let label = next.map_or_else(|| "Fully upgraded".to_owned(), |to| format!("Upgrade to {}", to.a_noun()));
        style.text_centered(&label, button, 14, Weight::Semibold, if upgradable { WHITE } else { style::TEXT_MUTED });
        if clicked && over_upgrade {
            self.error = None;
            change = Some(Change::Upgrade(yard.id));
        }

        if let Some(error) = &self.error {
            style.text(error, inner_x, button.bottom() + 20.0, 12, Weight::Regular, style::ERROR);
        }

        // The button that closes it.
        let close = Rect::new(inner_x, card.bottom() - PADDING - BUTTON_HEIGHT, inner_w, BUTTON_HEIGHT);
        let over_close = close.contains(mouse);
        style::rounded_rect(close, 8.0, if over_close { style::BORDER_STRONG } else { style::HOVER });
        style.text_centered("Close", close, 14, Weight::Semibold, style::TEXT);

        // A click on Close, or on the dimmed window outside the card, or Enter, closes the popup.
        let enter = is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter);
        if change.is_none() && (enter || (clicked && (over_close || !card.contains(mouse)))) {
            self.shipyard = None;
        }
        change
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::Date;
    use crate::shipyard::ShipyardKind;
    use nworld_map::world::Resource;

    fn day() -> DaySave {
        let mut day = DaySave::new(Date::FIRST, "Assets/map.nworld");
        day.found_country("Aldoria").unwrap();
        let id = day.countries[0].id;
        day.add_money(id, 20_000).unwrap();
        for resource in Resource::ALL {
            day.add_resource(id, resource, 2_000).unwrap();
        }
        day.build_shipyard(ShipyardKind::Small, 0.0, 0.0, None).unwrap();
        day
    }

    #[test]
    fn it_is_closed_until_opened_and_esc_closes_it() {
        assert!(!ShipyardPopup::default().is_open());
        // (Opening it reads the game's clock, which a test doesn't have, so it is made as open.)
        let mut popup = ShipyardPopup { shipyard: Some(ShipyardId::new()), ..ShipyardPopup::default() };
        assert!(popup.is_open());
        popup.escape();
        assert!(!popup.is_open());
    }

    #[test]
    fn a_failure_is_kept_to_show() {
        let mut popup = ShipyardPopup { shipyard: Some(ShipyardId::new()), ..ShipyardPopup::default() };
        popup.fail("no".to_owned());
        assert_eq!(popup.error.as_deref(), Some("no"));
    }

    #[test]
    fn an_upgrade_does_what_it_says_to_the_day() {
        let mut day = day();
        let id = day.shipyards[0].id;
        Change::Upgrade(id).apply(&mut day).unwrap();
        assert_eq!(day.shipyard(id).unwrap().kind, ShipyardKind::Large);
        assert!(Change::Upgrade(id).apply(&mut day).is_err(), "it is as large as it gets");
        assert!(Change::Upgrade(ShipyardId::new()).apply(&mut day).is_err(), "it is not there");
    }
}
