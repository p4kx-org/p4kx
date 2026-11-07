// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, RwLock};

use p4kx_foundation::ArchiveKey;

use crate::key_demand::KeyDemand;

/// The outcome of checking a key: it passed, or it failed on the named encrypted Entry.
pub(crate) type Outcome = Result<(), String>;

#[derive(Default)]
struct State {
    key: Option<ArchiveKey>,
    outcome: Option<Outcome>,
    wanted: AtomicBool,
}

/// The Archive key shared by an Archive and the containers opened from it, with the outcome of
/// checking it, so that a wrong key is found once and not again for every Entry.
#[derive(Clone, Default)]
pub(crate) struct KeySlot {
    state: Arc<RwLock<State>>,
    turn: Arc<Mutex<()>>,
    demand: KeyDemand,
}

impl KeySlot {
    /// Takes the turn to check the key, so that threads that open Entries at once check it once.
    pub(crate) fn check_turn(&self) -> MutexGuard<'_, ()> {
        self.turn.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Returns the handle to the record of the encrypted Entries that no key opened.
    pub(crate) const fn demand(&self) -> &KeyDemand {
        &self.demand
    }

    pub(crate) fn with(key: Option<ArchiveKey>) -> Self {
        let slot = Self::default();
        if let Some(key) = key {
            slot.set(key);
        }
        slot
    }

    pub(crate) fn set(&self, key: ArchiveKey) {
        *self.state.write().unwrap_or_else(PoisonError::into_inner) = State {
            key: Some(key),
            ..State::default()
        };
    }

    /// Notes that an encrypted Entry was needed while no key fit.
    pub(crate) fn want(&self) {
        self.state
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .wanted
            .store(true, Ordering::Relaxed);
    }

    /// Returns whether a key was wanted since the last call.
    pub(crate) fn take_wanted(&self) -> bool {
        self.state
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .wanted
            .swap(false, Ordering::Relaxed)
    }

    pub(crate) fn clear(&self) {
        *self.state.write().unwrap_or_else(PoisonError::into_inner) = State::default();
    }

    pub(crate) fn is_set(&self) -> bool {
        self.state
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .key
            .is_some()
    }

    /// Returns the key and the outcome of checking it, if it has been checked.
    pub(crate) fn get(&self) -> Option<(ArchiveKey, Option<Outcome>)> {
        let state = self.state.read().unwrap_or_else(PoisonError::into_inner);
        state.key.clone().map(|key| (key, state.outcome.clone()))
    }

    /// Records the outcome of checking `key`, unless another key was set meanwhile.
    pub(crate) fn record(&self, key: &ArchiveKey, outcome: Outcome) {
        let mut state = self.state.write().unwrap_or_else(PoisonError::into_inner);
        if state.key.as_ref() == Some(key) {
            if outcome.is_err() {
                state.wanted.store(true, Ordering::Relaxed);
            }
            state.outcome = Some(outcome);
        }
    }
}

impl fmt::Debug for KeySlot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KeySlot")
            .field("set", &self.is_set())
            .finish()
    }
}
