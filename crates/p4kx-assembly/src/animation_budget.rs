// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The budget of values a GLB may hold for animations, and the rule that decides whether the bone
//! keys of an animation fit it. The Armour and the creature Assembly both fit animations with it.

use p4kx_model::Animation;

/// The floats one key writes at most: its time and a rotation (four) or a translation (three).
const VALUES_PER_KEY: usize = 5;

/// What the bone keys of one animation come to against a budget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum KeyFit {
    /// No track names a bone of the skeleton.
    NoBone,
    /// The keys cost more than is left; the text says so and starts with `limit`.
    OverBudget(String),
    /// The keys fit and cost this many values.
    Fits(usize),
}

/// The values still available to the animations of one export, and the values spent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ValueBudget {
    remaining: usize,
    spent: usize,
}

impl ValueBudget {
    /// Starts a budget of `total` values.
    pub(crate) const fn new(total: usize) -> Self {
        Self {
            remaining: total,
            spent: 0,
        }
    }

    /// Returns the values not yet spent.
    pub(crate) const fn remaining(&self) -> usize {
        self.remaining
    }

    /// Returns the values spent.
    pub(crate) const fn spent(&self) -> usize {
        self.spent
    }

    /// Spends `cost` values, at most what is left.
    pub(crate) const fn spend(&mut self, cost: usize) {
        self.remaining = self.remaining.saturating_sub(cost);
        self.spent = self.spent.saturating_add(cost);
    }

    /// Decides whether the bone keys of `animation` fit, given whether any of its tracks names a
    /// bone of the skeleton (`has_bone`); spends nothing.
    pub(crate) fn fit_keys(&self, animation: &Animation, has_bone: bool) -> KeyFit {
        if !has_bone {
            return KeyFit::NoBone;
        }
        let cost = key_cost(animation);
        if cost > self.remaining {
            KeyFit::OverBudget(limit_reason(cost, self.remaining))
        } else {
            KeyFit::Fits(cost)
        }
    }
}

/// Returns the values the bone keys of `animation` write at most.
pub(crate) fn key_cost(animation: &Animation) -> usize {
    animation.key_count().saturating_mul(VALUES_PER_KEY)
}

/// Returns the reason an animation of `cost` values is left out when `remaining` are left; it
/// starts with `limit`, as the issue summary expects.
pub(crate) fn limit_reason(cost: usize, remaining: usize) -> String {
    format!("limit: the animation needs {cost} values, {remaining} of the budget are left")
}

#[cfg(test)]
mod tests {
    use p4kx_chunks::ChunkFile;
    use p4kx_model::Animation;
    use p4kx_testkit::{TestAnimation, TestTrack};

    use super::{KeyFit, ValueBudget};

    /// An animation of one track with `keys` rotation keys.
    fn animation(keys: u16) -> Animation {
        let frames: Vec<(u16, [f32; 4])> = (0..keys).map(|at| (at, [0.0, 0.0, 0.0, 1.0])).collect();
        let source = TestAnimation::new("a.caf", 30).track(TestTrack::new(1).rotations(&frames));
        let file = ChunkFile::parse(source.caf()).expect("a chunk file");
        Animation::read_caf("a.caf", &file).expect("an animation")
    }

    #[test]
    fn keys_that_fit_cost_five_values_each_and_the_check_spends_nothing() {
        let budget = ValueBudget::new(10);

        assert_eq!(budget.fit_keys(&animation(2), true), KeyFit::Fits(10));
        assert_eq!(budget.remaining(), 10);
    }

    #[test]
    fn keys_over_the_budget_give_a_limit_reason_and_a_missing_bone_wins() {
        let mut budget = ValueBudget::new(10);
        budget.spend(5);

        assert!(matches!(
            budget.fit_keys(&animation(2), true),
            KeyFit::OverBudget(reason) if reason.starts_with("limit")
        ));
        assert_eq!(budget.fit_keys(&animation(2), false), KeyFit::NoBone);
        assert_eq!((budget.remaining(), budget.spent()), (5, 5));
    }

    #[test]
    fn spending_more_than_is_left_stops_at_zero() {
        let mut budget = ValueBudget::new(3);

        budget.spend(8);

        assert_eq!((budget.remaining(), budget.spent()), (0, 8));
    }
}
