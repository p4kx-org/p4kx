// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::catalog_read::original::{OriginalValueFact, OriginalValuePart};

/// A selection minted only by its actual same-frame reference reader.
pub(crate) struct TargetSelection<'fields, 'scope, 'read, 'source> {
    pub(super) parent: Option<&'fields OriginalValueFact<'scope, 'read, 'source>>,
    pub(super) caller: Option<&'fields OriginalValueFact<'scope, 'read, 'source>>,
    pub(super) kind: TargetSelectionKind<'fields, 'scope, 'read, 'source>,
    pub(super) resolved: bool,
    pub(super) selectors: Vec<(
        &'fields OriginalValueFact<'scope, 'read, 'source>,
        OriginalValuePart,
    )>,
    pub(super) retained_root: Option<&'fields OriginalValueFact<'scope, 'read, 'source>>,
}

/// Read-only interpretation inputs, which cannot construct a selection receipt.
#[derive(Clone, Copy)]
pub(crate) enum TargetSelectionKind<'fields, 'scope, 'read, 'source> {
    Absent,
    Null,
    Unreadable,
    WrongShape,
    WrongStorage,
    OutsideOwner,
    RecordTarget,
    Unresolved,
    Instance {
        owner: &'fields OriginalValueFact<'scope, 'read, 'source>,
        field: Option<&'fields OriginalValueFact<'scope, 'read, 'source>>,
    },
}

impl<'fields, 'scope, 'read, 'source> TargetSelection<'fields, 'scope, 'read, 'source> {
    pub(crate) const fn retained_root(
        &self,
    ) -> Option<&'fields OriginalValueFact<'scope, 'read, 'source>> {
        self.retained_root
    }
    pub(crate) const fn kind(&self) -> TargetSelectionKind<'fields, 'scope, 'read, 'source> {
        self.kind
    }
}
