// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::contract::GameplayPropertyDefinition;

/// One original read's catalog retention independently of its public interpreted result.
pub(crate) enum GameplayPropertyInput<'source> {
    Selected(Box<GameplayPropertyDefinition<'source>>),
    Leftover,
    Dependency,
}

/// Canonical physical definitions and their measured intentional exclusions.
#[derive(Default)]
pub(crate) struct GameplayPropertyRows<'source> {
    pub(crate) definitions: Vec<GameplayPropertyDefinition<'source>>,
    pub(crate) skipped: usize,
}

impl<'source> GameplayPropertyRows<'source> {
    pub(crate) fn add(&mut self, input: Option<GameplayPropertyInput<'source>>) {
        match input {
            Some(GameplayPropertyInput::Selected(definition)) => self.definitions.push(*definition),
            Some(GameplayPropertyInput::Leftover) => {
                self.skipped = self.skipped.saturating_add(1);
            }
            Some(GameplayPropertyInput::Dependency) | None => {}
        }
    }

    pub(crate) fn order(&mut self) {
        self.definitions
            .sort_by_cached_key(|row| row.physical_guid.text());
    }
}
