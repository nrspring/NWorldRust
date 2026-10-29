//! Undo history for terrain edits.
//!
//! Each step stores only the samples it changed, with their previous heights and water levels,
//! so a step that touches a small part of the map costs little memory however large the map is.

use std::collections::VecDeque;

use crate::world::Resource;

/// How many steps can be undone.
pub const MAX_STEPS: usize = 10;

struct Step {
    /// (sample index, height before the step, water level before the step) for every sample the
    /// step changed.
    changes: Vec<(u32, f32, f32)>,
    /// For each resource, (sample index, density before the step) for every sample the step
    /// changed.
    resources: [Vec<(u32, u8)>; 2],
}

#[derive(Default)]
pub struct History {
    steps: VecDeque<Step>,
}

impl History {
    /// Records an edit that turned the heights `before` and water levels `water_before` into
    /// `after` and `water_after`. An edit that changed nothing is not recorded. Once there are
    /// more than [`MAX_STEPS`] steps, the oldest is forgotten.
    pub fn record(&mut self, before: &[f32], water_before: &[f32], after: &[f32], water_after: &[f32]) {
        debug_assert_eq!(before.len(), after.len());
        debug_assert_eq!(before.len(), water_before.len());
        debug_assert_eq!(before.len(), water_after.len());
        let changes: Vec<(u32, f32, f32)> = (0..before.len())
            .filter(|&i| before[i] != after[i] || water_before[i] != water_after[i])
            .map(|i| (i as u32, before[i], water_before[i]))
            .collect();
        if changes.is_empty() {
            return;
        }
        self.push(Step { changes, resources: Default::default() });
    }

    /// Records an edit that turned the `kind` densities `before` into `after`, with the same
    /// rules as [`record`](Self::record).
    pub fn record_resource(&mut self, kind: Resource, before: &[u8], after: &[u8]) {
        debug_assert_eq!(before.len(), after.len());
        let changed: Vec<(u32, u8)> = (0..before.len()).filter(|&i| before[i] != after[i]).map(|i| (i as u32, before[i])).collect();
        if !changed.is_empty() {
            let mut resources: [Vec<(u32, u8)>; 2] = Default::default();
            resources[kind.index()] = changed;
            self.push(Step { changes: Vec::new(), resources });
        }
    }

    fn push(&mut self, step: Step) {
        self.steps.push_back(step);
        if self.steps.len() > MAX_STEPS {
            self.steps.pop_front();
        }
    }

    /// Undoes the most recent step on `heights`, `water` and the resource densities. Returns
    /// false if there was nothing to undo.
    pub fn undo(&mut self, heights: &mut [f32], water: &mut [f32], resources: &mut [Vec<u8>; 2]) -> bool {
        let Some(step) = self.steps.pop_back() else {
            return false;
        };
        for (i, h, w) in step.changes {
            heights[i as usize] = h;
            water[i as usize] = w;
        }
        for (layer, changed) in resources.iter_mut().zip(step.resources) {
            for (i, d) in changed {
                layer[i as usize] = d;
            }
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

    /// Undoes a step of terrain edits, with no resources.
    fn undo(history: &mut History, heights: &mut [f32], water: &mut [f32]) -> bool {
        let n = heights.len();
        history.undo(heights, water, &mut [vec![0; n], vec![0; n]])
    }

    #[test]
    fn resource_edits_are_steps_of_their_own_and_undo_only_their_resource() {
        let mut history = History::default();
        let (mut heights, mut water) = (vec![0.0f32; 6], vec![0.0f32; 6]);
        let before = vec![0u8, 10, 20, 30, 40, 50];
        let mut stone = before.clone();
        stone[1] = 200;
        stone[4] = 0;
        history.record_resource(Resource::Stone, &before, &stone);
        history.record_resource(Resource::Stone, &stone, &stone);
        assert_eq!(history.steps(), 1, "an edit that changed nothing isn't a step");
        let mut resources = [before.clone(), stone];
        assert!(history.undo(&mut heights, &mut water, &mut resources));
        assert_eq!(resources[1], before);
        assert_eq!(resources[0], before, "the forest was never touched");
        assert!(!history.undo(&mut heights, &mut water, &mut resources));
    }

    #[test]
    fn undo_restores_each_state_exactly() {
        let mut history = History::default();
        let mut heights = vec![-200.0f32; 100];
        let mut water = vec![0.0f32; 100];
        let mut states = vec![(heights.clone(), water.clone())];
        for step in 0..MAX_STEPS {
            let (before, water_before) = (heights.clone(), water.clone());
            for (i, h) in heights.iter_mut().enumerate() {
                if (i + step) % 7 == 0 {
                    *h = step as f32;
                }
            }
            // Every other step moves some water level too.
            if step % 2 == 1 {
                water[step] = 10.0 + step as f32;
            }
            history.record(&before, &water_before, &heights, &water);
            states.push((heights.clone(), water.clone()));
        }
        assert_eq!(history.steps(), MAX_STEPS);
        for (expected_heights, expected_water) in states.iter().rev().skip(1) {
            assert!(undo(&mut history, &mut heights, &mut water));
            assert_eq!(&heights, expected_heights);
            assert_eq!(&water, expected_water);
        }
        assert!(!undo(&mut history, &mut heights, &mut water));
    }

    #[test]
    fn a_change_to_only_the_water_is_a_step_of_its_own() {
        let mut history = History::default();
        let heights = vec![5.0f32; 4];
        let mut water = vec![0.0f32; 4];
        let before = water.clone();
        water[2] = 5.5;
        history.record(&heights, &before, &heights, &water);
        assert_eq!(history.steps(), 1);
        let mut heights = heights;
        assert!(undo(&mut history, &mut heights, &mut water));
        assert_eq!(water, before);
    }

    #[test]
    fn keeps_only_the_last_ten_steps() {
        let mut history = History::default();
        let mut heights = vec![0.0f32; 8];
        let mut water = vec![0.0f32; 8];
        let mut states = vec![heights.clone()];
        for step in 0..MAX_STEPS + 3 {
            let before = heights.clone();
            heights[step % 8] += 1.0;
            history.record(&before, &water, &heights, &water);
            states.push(heights.clone());
        }
        assert_eq!(history.steps(), MAX_STEPS);
        for _ in 0..MAX_STEPS {
            assert!(undo(&mut history, &mut heights, &mut water));
        }
        // Ten undos reach the state three steps after the start; the first three steps are gone.
        assert_eq!(heights, states[3]);
        assert!(!undo(&mut history, &mut heights, &mut water));
    }

    #[test]
    fn edits_that_change_nothing_are_not_recorded() {
        let mut history = History::default();
        let heights = vec![1.0f32; 4];
        history.record(&heights, &heights, &heights, &heights);
        assert_eq!(history.steps(), 0);
    }
}
