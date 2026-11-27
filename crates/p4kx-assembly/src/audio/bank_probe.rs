// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::rc::{Rc, Weak};

use p4kx_audio::Soundbank;

/// Opt-in unit observations of completed foreign loads and actual selected allocations.
#[derive(Debug, Default)]
pub(super) struct BankProbe {
    pub(super) reads: Vec<u32>,
    pub(super) selections: Vec<Selection>,
}

/// One selected original object without retaining its parsed bank.
#[derive(Debug)]
pub(super) struct Selection {
    pub(super) bank: u32,
    pub(super) object: u32,
    pub(super) allocation: Weak<Soundbank<Vec<u8>>>,
}

impl BankProbe {
    pub(super) fn selected(&mut self, position: u32, object: u32, bank: &Rc<Soundbank<Vec<u8>>>) {
        self.selections.push(Selection {
            bank: position,
            object,
            allocation: Rc::downgrade(bank),
        });
    }

    pub(super) fn selection(&self, bank: u32, object: u32) -> Option<&Selection> {
        self.selections
            .iter()
            .find(|selected| selected.bank == bank && selected.object == object)
    }
}
