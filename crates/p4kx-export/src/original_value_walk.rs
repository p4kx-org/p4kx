// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, Field, Value};

use crate::CatalogError;

use super::super::OriginalRecordScope;
use super::owned_walk::{OwnedValueEvent, OwnedValueWalk};
use super::tokens::{OriginalContainerKind, OriginalValueEdge};
use super::visit::OwnedValueVisit;

#[path = "original_value_fact.rs"]
mod fact;

pub(crate) use fact::OriginalValueFact;

/// One actual field or array value bound only by its own admitted Scope walk.
pub(crate) struct OriginalValueSite<'visit, 'scope, 'read, 'source> {
    scope: &'scope OriginalRecordScope<'read, 'source>,
    visit: OwnedValueVisit<'visit, 'read, 'source>,
}

impl<'scope, 'read, 'source> OriginalValueSite<'_, 'scope, 'read, 'source> {
    pub(crate) const fn value(&self) -> &'read Value<'source> {
        self.visit.value()
    }
    pub(crate) const fn field(&self) -> &'read Field<'source> {
        self.visit.field()
    }
    pub(crate) const fn ordinal(&self) -> u64 {
        self.visit.ordinal()
    }
    pub(crate) const fn record_index(&self) -> u32 {
        self.scope.record().index()
    }
    pub(crate) const fn parent_ordinal(&self) -> Option<u64> {
        self.visit.parent_ordinal()
    }
    pub(crate) const fn edge(&self) -> OriginalValueEdge {
        self.visit.edge()
    }
    pub(crate) const fn owner_type(&self) -> &'source str {
        self.visit.owner_type()
    }
    pub(crate) fn logical_path(&self) -> &str {
        self.visit.logical_path()
    }

    pub(crate) const fn neutral(&self) -> &OwnedValueVisit<'_, 'read, 'source> {
        &self.visit
    }

    pub(crate) const fn fact(&self) -> OriginalValueFact<'scope, 'read, 'source> {
        OriginalValueFact::capture(self)
    }
}

/// One actual value visit or completion bound to the same original Scope.
pub(crate) enum OriginalValueEvent<'visit, 'scope, 'read, 'source> {
    Value(OriginalValueSite<'visit, 'scope, 'read, 'source>),
    EndContainer {
        ordinal: u64,
        kind: OriginalContainerKind,
    },
}

/// A Scope-owned adapter that alone binds its own neutral visits to source authority.
pub(crate) struct OriginalValueWalk<'scope, 'read, 'source> {
    scope: &'scope OriginalRecordScope<'read, 'source>,
    owned: OwnedValueWalk<'read, 'source, 'scope>,
}

impl<'read, 'source> OriginalRecordScope<'read, 'source> {
    pub(crate) fn walk<'scope>(
        &'scope self,
        cancel: Option<&'scope CancelToken>,
    ) -> OriginalValueWalk<'scope, 'read, 'source> {
        OriginalValueWalk {
            scope: self,
            owned: OwnedValueWalk::admitted(self.tree(), cancel),
        }
    }
}

impl<'scope, 'read, 'source> OriginalValueWalk<'scope, 'read, 'source> {
    pub(crate) fn next_event(
        &mut self,
    ) -> Result<Option<OriginalValueEvent<'_, 'scope, 'read, 'source>>, CatalogError> {
        let scope = self.scope;
        Ok(self.owned.next_event()?.map(|event| match event {
            OwnedValueEvent::Value(visit) => {
                OriginalValueEvent::Value(OriginalValueSite { scope, visit })
            }
            OwnedValueEvent::EndContainer { ordinal, kind } => {
                OriginalValueEvent::EndContainer { ordinal, kind }
            }
        }))
    }
}
