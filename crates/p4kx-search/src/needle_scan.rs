// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::VecDeque;
use std::ops::Range;

use crate::needle;

#[derive(Clone, Copy)]
struct Position {
    at: u64,
    first: bool,
}

/// Carries literal-prefix progress between original Unicode characters.
pub(crate) struct NeedleScan<'needle> {
    pattern: &'needle [char],
    prefix: &'needle [usize],
    positions: VecDeque<Position>,
    matched: usize,
}

impl<'needle> NeedleScan<'needle> {
    pub(crate) fn new(pattern: &'needle [char], prefix: &'needle [usize]) -> Self {
        Self {
            pattern,
            prefix,
            positions: VecDeque::with_capacity(pattern.len()),
            matched: 0,
        }
    }

    pub(crate) fn reset(&mut self) {
        self.positions.clear();
        self.matched = 0;
    }

    pub(crate) fn push(&mut self, character: char, at: u64) -> Option<Range<u64>> {
        for (index, lower) in needle::fold(character).enumerate() {
            if self.positions.len() == self.pattern.len() {
                self.positions.pop_front();
            }
            self.positions.push_back(Position {
                at,
                first: index == 0,
            });
            while self.matched > 0 && !self.extends(lower) {
                self.matched = self
                    .prefix
                    .get(self.matched - 1)
                    .copied()
                    .unwrap_or_default();
            }
            if self.extends(lower) {
                self.matched += 1;
            }
            if self.matched == self.pattern.len() {
                let start = self.positions.len().checked_sub(self.matched)?;
                return Some(self.positions.get(start)?.at..at + character.len_utf8() as u64);
            }
        }
        None
    }

    fn extends(&self, character: char) -> bool {
        self.pattern.get(self.matched) == Some(&character)
            && self
                .positions
                .len()
                .checked_sub(self.matched + 1)
                .and_then(|index| self.positions.get(index))
                .is_some_and(|position| position.first)
    }

    pub(crate) const fn memory(length: usize) -> u64 {
        length.saturating_mul(size_of::<Position>()) as u64
    }
}

/// Compiles literal fallbacks independently of source-character boundaries.
pub(crate) fn prefix(pattern: &[char]) -> Vec<usize> {
    let mut prefix = vec![0; pattern.len()];
    for index in 1..pattern.len() {
        let mut matched = prefix.get(index - 1).copied().unwrap_or_default();
        while matched > 0 && pattern.get(matched) != pattern.get(index) {
            matched = prefix.get(matched - 1).copied().unwrap_or_default();
        }
        if pattern.get(matched) == pattern.get(index) {
            matched += 1;
        }
        if let Some(slot) = prefix.get_mut(index) {
            *slot = matched;
        }
    }
    prefix
}
