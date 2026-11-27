// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::sync::Arc;

use p4kx_extraction::{FieldOrigin, SourceUseRef};
use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Serialize, Serializer};

use crate::variant_decision::VariantDecision;
use crate::{CatalogError, ExportAccountingError, schema_fields};

/// A physical variant cohort's shared contributing uses in this result inventory.
#[derive(Debug, PartialEq, Eq)]
pub struct VariantSourceUseSet {
    id: u32,
    origin: FieldOrigin,
}

impl VariantSourceUseSet {
    pub(crate) const fn new(id: u32, origin: FieldOrigin) -> Self {
        Self { id, origin }
    }

    /// Returns the smallest original Record index in this cohort's shared comparison scope.
    #[must_use]
    pub const fn id(&self) -> u32 {
        self.id
    }

    /// Returns the sorted contributing use references in this result's original inventory.
    #[must_use]
    pub fn source_uses_refs(&self) -> &[SourceUseRef] {
        self.origin.source_uses_refs()
    }

    pub(crate) const fn origin(&self) -> &FieldOrigin {
        &self.origin
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = schema_fields::every_field_required)]
struct SourceUseSetWire<'source> {
    id: u32,
    #[schemars(extend("uniqueItems" = true))]
    source_uses_refs: &'source [SourceUseRef],
}

impl Serialize for VariantSourceUseSet {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        SourceUseSetWire {
            id: self.id,
            source_uses_refs: self.source_uses_refs(),
        }
        .serialize(serializer)
    }
}

impl JsonSchema for VariantSourceUseSet {
    fn schema_name() -> Cow<'static, str> {
        "VariantSourceUseSet".into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        SourceUseSetWire::json_schema(generator)
    }
}

/// The shared variant evidence sets referenced by emitted rows in one result.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VariantSourceUseSets(Vec<Arc<VariantSourceUseSet>>);

impl VariantSourceUseSets {
    pub(crate) fn from_decisions<'decision>(
        decisions: impl IntoIterator<Item = &'decision VariantDecision>,
    ) -> Result<Self, CatalogError> {
        let mut sets = BTreeMap::new();
        for set in decisions
            .into_iter()
            .filter_map(VariantDecision::shared_uses)
        {
            match sets.entry(set.id()) {
                Entry::Vacant(entry) => {
                    entry.insert(Arc::clone(set));
                }
                Entry::Occupied(entry) if !Arc::ptr_eq(entry.get(), set) => {
                    return Err(ExportAccountingError::ConflictingSourceDatum.into());
                }
                Entry::Occupied(_) => {}
            }
        }
        Ok(Self(sets.into_values().collect()))
    }

    /// Iterates shared evidence in physical group ID order.
    pub fn iter_sets(&self) -> impl ExactSizeIterator<Item = &VariantSourceUseSet> {
        self.0.iter().map(Arc::as_ref)
    }

    /// Returns the shared evidence identified by a variant's source-use-set reference.
    #[must_use]
    pub fn get(&self, id: u32) -> Option<&VariantSourceUseSet> {
        let index = self.0.binary_search_by_key(&id, |set| set.id()).ok()?;
        self.0.get(index).map(Arc::as_ref)
    }

    /// Returns whether the emitted rows have any shared variant evidence.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Serialize for VariantSourceUseSets {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.iter_sets())
    }
}

impl JsonSchema for VariantSourceUseSets {
    fn schema_name() -> Cow<'static, str> {
        "VariantSourceUseSets".into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        Vec::<VariantSourceUseSet>::json_schema(generator)
    }
}
