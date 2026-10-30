//! Placing something on the map with a click. A press and release of the left mouse button that
//! hardly moves is a click; one that moves is the player dragging the map, which is for the camera.

use macroquad::prelude::*;

/// How far the mouse may move between press and release, in pixels, and still be a click.
const CLICK_SLOP: f32 = 6.0;

#[derive(Default)]
pub struct ClickTracker {
    /// Where the left button went down, if it is down and the press counts.
    pressed_at: Option<Vec2>,
}

impl ClickTracker {
    /// Call every frame. True on the frame a click is completed. A press that starts while the mouse
    /// is not `free`, as when it is over the interface, is never a click.
    pub fn update(&mut self, free: bool) -> bool {
        self.step(is_mouse_button_pressed(MouseButton::Left), is_mouse_button_released(MouseButton::Left), Vec2::from(mouse_position()), free)
    }

    fn step(&mut self, pressed: bool, released: bool, mouse: Vec2, free: bool) -> bool {
        if pressed {
            self.pressed_at = free.then_some(mouse);
        }
        if released {
            return self.pressed_at.take().is_some_and(|start| start.distance(mouse) <= CLICK_SLOP);
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_press_and_release_in_place_is_a_click() {
        let mut clicks = ClickTracker::default();
        assert!(!clicks.step(true, false, vec2(100.0, 100.0), true));
        assert!(clicks.step(false, true, vec2(102.0, 101.0), true));
    }

    #[test]
    fn a_drag_is_not_a_click() {
        let mut clicks = ClickTracker::default();
        clicks.step(true, false, vec2(100.0, 100.0), true);
        assert!(!clicks.step(false, true, vec2(160.0, 100.0), true));
    }

    #[test]
    fn a_press_over_the_interface_is_not_a_click() {
        let mut clicks = ClickTracker::default();
        clicks.step(true, false, vec2(100.0, 100.0), false);
        assert!(!clicks.step(false, true, vec2(100.0, 100.0), true));
    }

    #[test]
    fn a_release_with_no_press_is_not_a_click() {
        let mut clicks = ClickTracker::default();
        assert!(!clicks.step(false, true, vec2(100.0, 100.0), true));
    }

    #[test]
    fn each_click_counts_once() {
        let mut clicks = ClickTracker::default();
        clicks.step(true, false, vec2(5.0, 5.0), true);
        assert!(clicks.step(false, true, vec2(5.0, 5.0), true));
        assert!(!clicks.step(false, false, vec2(5.0, 5.0), true));
        assert!(!clicks.step(false, true, vec2(5.0, 5.0), true));
    }
}
