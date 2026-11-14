// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use super::{FixtureGuard, FixtureImage, FixtureValue};

/// A fixed-scope native Registry fixture bound to its acknowledged runner.
#[derive(Debug)]
pub struct FixtureSession {
    guard: FixtureGuard,
}

impl FixtureSession {
    /// Retains an independently validated disposable-runner guard.
    #[must_use]
    pub const fn new(guard: FixtureGuard) -> Self {
        Self { guard }
    }

    /// Returns the identity and fixed task directory of this runner.
    #[must_use]
    pub const fn guard(&self) -> &FixtureGuard {
        &self.guard
    }

    /// Captures exact fixed-value bytes and key presence in one native transaction.
    ///
    /// # Errors
    /// Returns runner refusal, byte limits or original native Registry errors.
    pub fn capture(&self) -> io::Result<FixtureImage> {
        super::registry::capture(&self.guard)
    }

    /// Seeds only PATH after comparing the complete expected fixed-value image.
    ///
    /// # Errors
    /// Returns runner, conflict, Registry or exact native read-back failures.
    pub fn seed_path(
        &self,
        expected: &FixtureImage,
        path: Option<FixtureValue>,
    ) -> io::Result<FixtureImage> {
        self.guard.active()?;
        let mut desired = expected.clone();
        if path.is_some() {
            let [_, installer, ownership, invocation] = desired.keys.values();
            desired.keys = super::FixtureKeys::new(
                super::KeyPresence::Present,
                installer,
                ownership,
                invocation,
            )?;
        }
        desired.path = path;
        super::registry::replace(&self.guard, expected, &desired)
    }

    /// Restores the recorded image atomically while retaining unrelated Registry contents.
    ///
    /// # Errors
    /// Returns runner, conflict, Registry or nonempty foreign-key failures without committing.
    pub fn restore(
        &self,
        expected: &FixtureImage,
        original: &FixtureImage,
    ) -> io::Result<FixtureImage> {
        self.guard.active()?;
        super::registry::replace(&self.guard, expected, original)
    }
}
