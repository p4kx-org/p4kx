// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::selection::Kind;

impl Kind {
    /// Returns the position of the kind in [`Kind::ALL`].
    pub(crate) const fn index(self) -> usize {
        self as usize
    }
}

const _: () = {
    let mut kinds: &[Kind] = &Kind::ALL;
    let mut index = 0;
    while let [kind, rest @ ..] = kinds {
        assert!(kind.index() == index, "catalog kind order");
        kinds = rest;
        index += 1;
    }
};
