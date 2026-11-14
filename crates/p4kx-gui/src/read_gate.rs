// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The gate between giving an export up while it reads DataForge and its thread leaving that
//! step, shared by the Record export and the P4KX catalog.

use std::sync::{Mutex, PoisonError};

/// Whether a run may still be given up while it reads DataForge.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum Gate {
    #[default]
    Reading,
    GivenUp,
    Past,
}

/// Decides between the window giving a run up while it reads DataForge and the run's thread
/// leaving the read step, so that a run given up never goes on.
#[derive(Debug, Default)]
pub(crate) struct ReadGate {
    gate: Mutex<Gate>,
}

impl ReadGate {
    /// Ends the reading of DataForge; returns `false` when the run was given up meanwhile.
    pub(crate) fn leave_reading(&self) -> bool {
        let mut gate = self.gate.lock().unwrap_or_else(PoisonError::into_inner);
        if *gate == Gate::GivenUp {
            return false;
        }
        *gate = Gate::Past;
        true
    }

    /// Gives the run up while it still reads DataForge; returns whether it was given up.
    pub(crate) fn give_up(&self) -> bool {
        let mut gate = self.gate.lock().unwrap_or_else(PoisonError::into_inner);
        if *gate == Gate::Past {
            return false;
        }
        *gate = Gate::GivenUp;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::ReadGate;

    #[test]
    fn a_run_given_up_while_reading_does_not_go_on() {
        let gate = ReadGate::default();

        let given_up = gate.give_up();

        assert!(given_up, "given up while reading");
        assert!(!gate.leave_reading(), "the thread stops after reading");
    }

    #[test]
    fn a_run_past_reading_is_not_given_up() {
        let gate = ReadGate::default();

        let left = gate.leave_reading();

        assert!(left, "reading ended");
        assert!(!gate.give_up(), "the run is waited for");
    }
}
