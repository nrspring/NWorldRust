//! Undo history for heightmap edits.
//!
//! Each step stores only the samples it changed, with their previous heights, so a step that
//! touches a small part of the map costs little memory however large the map is.

use std::collections::VecDeque;

/// How many steps can be undone.
pub const MAX_STEPS: usize = 10;

struct Step {
    /// (sample index, height before the step) for every sample the step changed.
    changes: Vec<(u32, f32)>,
}

#[derive(Default)]
pub struct History {
    steps: VecDeque<Step>,
}

impl History {
    /// Records an edit that turned the heights `before` into `after`. An edit that changed
    /// nothing is not recorded. Once there are more than [`MAX_STEPS`] steps, the oldest is
    /// forgotten.
    pub fn record(&mut self, before: &[f32], after: &[f32]) {
        debug_assert_eq!(before.len(), after.len());
        let changes: Vec<(u32, f32)> = before
            .iter()
            .zip(after)
            .enumerate()
            .filter(|(_, (b, a))| b != a)
            .map(|(i, (&b, _))| (i as u32, b))
            .collect();
        if changes.is_empty() {
            return;
        }
        self.steps.push_back(Step { changes });
        if self.steps.len() > MAX_STEPS {
            self.steps.pop_front();
        }
    }

    /// Undoes the most recent step on `heights`. Returns false if there was nothing to undo.
    pub fn undo(&mut self, heights: &mut [f32]) -> bool {
        let Some(step) = self.steps.pop_back() else {
            return false;
        };
        for (i, h) in step.changes {
            heights[i as usize] = h;
        }
        true
    }

    /// How many steps can still be undone.
    pub fn steps(&self) -> usize {
        self.steps.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undo_restores_each_state_exactly() {
        let mut history = History::default();
        let mut heights = vec![-200.0f32; 100];
        let mut states = vec![heights.clone()];
        for step in 0..MAX_STEPS {
            let before = heights.clone();
            for (i, h) in heights.iter_mut().enumerate() {
                if (i + step) % 7 == 0 {
                    *h = step as f32;
                }
            }
            history.record(&before, &heights);
            states.push(heights.clone());
        }
        assert_eq!(history.steps(), MAX_STEPS);
        for expected in states.iter().rev().skip(1) {
            assert!(history.undo(&mut heights));
            assert_eq!(&heights, expected);
        }
        assert!(!history.undo(&mut heights));
    }

    #[test]
    fn keeps_only_the_last_ten_steps() {
        let mut history = History::default();
        let mut heights = vec![0.0f32; 8];
        let mut states = vec![heights.clone()];
        for step in 0..MAX_STEPS + 3 {
            let before = heights.clone();
            heights[step % 8] += 1.0;
            history.record(&before, &heights);
            states.push(heights.clone());
        }
        assert_eq!(history.steps(), MAX_STEPS);
        for _ in 0..MAX_STEPS {
            assert!(history.undo(&mut heights));
        }
        // Ten undos reach the state three steps after the start; the first three steps are gone.
        assert_eq!(heights, states[3]);
        assert!(!history.undo(&mut heights));
    }

    #[test]
    fn edits_that_change_nothing_are_not_recorded() {
        let mut history = History::default();
        let heights = vec![1.0f32; 4];
        history.record(&heights, &heights);
        assert_eq!(history.steps(), 0);
    }
}
