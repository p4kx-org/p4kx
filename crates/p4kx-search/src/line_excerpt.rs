// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::VecDeque;
use std::ops::Range;

use crate::Excerpt;
use crate::excerpt::{CONTEXT_AFTER, CONTEXT_BEFORE, ExcerptParts, MAX_MATCH_CHARS};

struct Captured {
    head: String,
    found: String,
    tail: String,
    head_cut: bool,
    found_cut: bool,
    tail_cut: bool,
    tail_chars: usize,
}

/// Retains only the source needed for one bounded first-match excerpt.
pub(crate) struct LineExcerpt {
    history: VecDeque<(u64, char)>,
    capacity: usize,
    captured: Option<Captured>,
}

impl LineExcerpt {
    pub(crate) fn new(needle_chars: usize) -> Self {
        let capacity = needle_chars.saturating_add(CONTEXT_BEFORE);
        Self {
            history: VecDeque::with_capacity(capacity),
            capacity,
            captured: None,
        }
    }

    pub(crate) fn reset(&mut self) {
        self.history.clear();
        self.captured = None;
    }

    pub(crate) const fn has_match(&self) -> bool {
        self.captured.is_some()
    }

    pub(crate) fn push(&mut self, character: char, at: u64) {
        if let Some(captured) = &mut self.captured {
            if !captured.found_cut {
                if captured.tail_chars < CONTEXT_AFTER {
                    captured.tail.push(character);
                    captured.tail_chars += 1;
                } else {
                    captured.tail_cut = true;
                }
            }
            return;
        }
        if self.history.len() == self.capacity {
            self.history.pop_front();
        }
        self.history.push_back((at, character));
    }

    pub(crate) fn capture(&mut self, matched: &Range<u64>) -> Option<()> {
        let start = self
            .history
            .iter()
            .position(|(at, _)| *at == matched.start)?;
        let head_start = start.saturating_sub(CONTEXT_BEFORE);
        let head = self
            .history
            .iter()
            .skip(head_start)
            .take(start - head_start)
            .map(|(_, character)| character)
            .collect();
        let found = self
            .history
            .iter()
            .skip(start)
            .take(MAX_MATCH_CHARS)
            .map(|(_, character)| character)
            .collect();
        let head_cut = self.history.get(head_start).is_some_and(|(at, _)| *at > 0);
        self.captured = Some(Captured {
            head,
            found,
            tail: String::new(),
            head_cut,
            found_cut: self.history.len().saturating_sub(start) > MAX_MATCH_CHARS,
            tail_cut: false,
            tail_chars: 0,
        });
        self.history.clear();
        Some(())
    }

    pub(crate) fn take(&mut self) -> Option<Excerpt> {
        self.captured.take().map(|captured| {
            Excerpt::from_parts(&ExcerptParts {
                head: &captured.head,
                found: &captured.found,
                tail: &captured.tail,
                head_cut: captured.head_cut,
                tail_cut: captured.found_cut || captured.tail_cut,
            })
        })
    }

    pub(crate) const fn memory(needle_chars: usize) -> u64 {
        let history = needle_chars
            .saturating_add(CONTEXT_BEFORE)
            .saturating_mul(size_of::<(u64, char)>());
        let text = (CONTEXT_BEFORE + MAX_MATCH_CHARS + CONTEXT_AFTER) * 4;
        history.saturating_add(text) as u64
    }
}
