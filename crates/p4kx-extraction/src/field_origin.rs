// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::sync::{Arc, OnceLock};

use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Serialize, Serializer};

use crate::projection_schema;
use crate::source_inventory::SourceEncryptionState;
use crate::source_use::{
    OriginUseId, SourceUse, SourceUseDescription, SourceUseRef, SourceUseRole,
};

#[path = "field_origin_refs.rs"]
mod refs;

#[cfg(test)]
#[path = "field_origin_shared_tests.rs"]
mod shared_tests;

use self::refs::OriginRefs;

/// Shared authoritative uses and the immediate filter index for one data value.
#[derive(Debug, Clone)]
pub struct FieldOrigin {
    fields: Arc<OriginFields>,
    identity: Option<Arc<()>>,
}

#[derive(Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = correlate_schema)]
struct OriginFields {
    contains_encrypted_source: Option<bool>,
    #[schemars(with = "Vec<SourceUseRef>")]
    source_uses_refs: OriginRefs,
    source_set_complete: bool,
}

impl OriginFields {
    fn empty(complete: bool) -> Arc<Self> {
        static COMPLETE: OnceLock<Arc<OriginFields>> = OnceLock::new();
        static INCOMPLETE: OnceLock<Arc<OriginFields>> = OnceLock::new();
        let cache = if complete { &COMPLETE } else { &INCOMPLETE };
        Arc::clone(cache.get_or_init(|| {
            Arc::new(Self {
                contains_encrypted_source: complete.then_some(false),
                source_uses_refs: OriginRefs::Flat(Vec::new()),
                source_set_complete: complete,
            })
        }))
    }

    fn from_refs(
        contains_encrypted_source: Option<bool>,
        references: Vec<SourceUseRef>,
        source_set_complete: bool,
    ) -> Arc<Self> {
        if references.is_empty() {
            debug_assert_eq!(
                contains_encrypted_source,
                source_set_complete.then_some(false),
                "an empty source set only reports encryption certainty through completeness"
            );
            return Self::empty(source_set_complete);
        }
        Arc::new(Self {
            contains_encrypted_source,
            source_uses_refs: OriginRefs::flat(references),
            source_set_complete,
        })
    }
}

/// A contributor set shared by values from the same source inventory.
#[derive(Debug)]
pub struct SharedFieldOrigin {
    identity: Arc<()>,
    contains_encrypted_source: Option<bool>,
    source_uses_refs: Arc<[SourceUseRef]>,
    source_set_complete: bool,
    fields: OnceLock<Arc<OriginFields>>,
}

impl SharedFieldOrigin {
    pub(crate) fn new<'a>(
        identity: Arc<()>,
        uses: impl IntoIterator<Item = &'a SourceUse>,
        complete: bool,
    ) -> Self {
        let facts = OriginFacts::measure(uses.into_iter().map(SourceUse::description), complete);
        Self {
            identity,
            contains_encrypted_source: facts.contains_encrypted_source,
            source_uses_refs: facts.references.into(),
            source_set_complete: facts.source_set_complete,
            fields: OnceLock::new(),
        }
    }

    pub(crate) fn belongs_to(&self, identity: &Arc<()>) -> bool {
        Arc::ptr_eq(&self.identity, identity)
    }

    fn fields_with(&self, mut local: OriginFacts) -> Arc<OriginFields> {
        local
            .references
            .retain(|reference| self.source_uses_refs.binary_search(reference).is_err());
        let source_set_complete = self.source_set_complete && local.source_set_complete;
        if self.source_uses_refs.is_empty() && local.references.is_empty() {
            return OriginFields::empty(source_set_complete);
        }
        let contains_encrypted_source = match (
            self.contains_encrypted_source,
            local.contains_encrypted_source,
        ) {
            (Some(true), _) | (_, Some(true)) => Some(true),
            (Some(false), Some(false)) if source_set_complete => Some(false),
            _ => None,
        };
        if local.references.is_empty()
            && source_set_complete == self.source_set_complete
            && contains_encrypted_source == self.contains_encrypted_source
        {
            return Arc::clone(self.fields.get_or_init(|| {
                Arc::new(OriginFields {
                    contains_encrypted_source,
                    source_uses_refs: OriginRefs::Basis(Arc::clone(&self.source_uses_refs)),
                    source_set_complete,
                })
            }));
        }
        Arc::new(OriginFields {
            contains_encrypted_source,
            source_uses_refs: OriginRefs::shared(&self.source_uses_refs, local.references),
            source_set_complete,
        })
    }
}

/// Correlates immediate-origin markers and requires distinct, nonempty references.
pub(crate) fn correlate_schema(schema: &mut Schema) {
    projection_schema::require_properties(schema);
    let constraint = schemars::json_schema!({
        "properties": {
            "sourceUsesRefs": {
                "uniqueItems": true,
                "items": {"type": "string", "minLength": 1}
            }
        },
        "allOf": [
            {
                "if": {
                    "properties": {"containsEncryptedSource": {"const": false}},
                    "required": ["containsEncryptedSource"]
                },
                "then": {"properties": {"sourceSetComplete": {"const": true}}}
            },
            {
                "if": {
                    "properties": {"sourceUsesRefs": {"maxItems": 0}},
                    "required": ["sourceUsesRefs"]
                },
                "then": {
                    "if": {
                        "properties": {"sourceSetComplete": {"const": true}},
                        "required": ["sourceSetComplete"]
                    },
                    "then": {"properties": {"containsEncryptedSource": {"const": false}}},
                    "else": {"properties": {"containsEncryptedSource": {"type": "null"}}}
                }
            }
        ]
    })
    .to_value();
    if let Some(all) = schema
        .get_mut("allOf")
        .and_then(|value| value.as_array_mut())
    {
        if !all.contains(&constraint) {
            all.push(constraint);
        }
    } else {
        schema.insert("allOf".into(), vec![constraint].into());
    }
}

struct OriginFacts {
    contains_encrypted_source: Option<bool>,
    references: Vec<SourceUseRef>,
    source_set_complete: bool,
}

impl OriginFacts {
    fn measure<'a>(
        uses: impl IntoIterator<Item = &'a SourceUseDescription>,
        complete: bool,
    ) -> Self {
        let mut accumulator = OriginAccumulator::new(complete);
        for description in uses {
            accumulator.observe(OriginContribution::from(description));
        }
        accumulator.into_facts()
    }
}

#[derive(Clone, Copy)]
pub(crate) struct OriginContribution<'a> {
    pub(crate) id: OriginUseId<'a>,
    pub(crate) role: SourceUseRole,
    pub(crate) encryption: SourceEncryptionState,
    pub(crate) opaque: bool,
}

impl<'a> From<&'a SourceUseDescription> for OriginContribution<'a> {
    fn from(description: &'a SourceUseDescription) -> Self {
        Self {
            id: OriginUseId::Description(description.id()),
            role: description.role(),
            encryption: description.effective_encryption(),
            opaque: description.is_opaque(),
        }
    }
}

pub(crate) struct OriginAccumulator {
    references: Vec<SourceUseRef>,
    encryption: SourceEncryptionState,
    complete: bool,
}

impl OriginAccumulator {
    pub(crate) const fn new(complete: bool) -> Self {
        Self {
            references: Vec::new(),
            encryption: SourceEncryptionState::Unencrypted,
            complete,
        }
    }

    pub(crate) const fn include_completeness(&mut self, complete: bool) {
        self.complete &= complete;
    }

    pub(crate) fn observe(&mut self, contribution: OriginContribution<'_>) {
        if contribution.role.contributes() {
            self.include_completeness(!contribution.opaque);
            self.references.push(contribution.id.retain());
            self.encryption = self.encryption.combine(contribution.encryption);
        }
    }

    pub(crate) fn finish(self, identity: Arc<()>) -> FieldOrigin {
        FieldOrigin::from_facts(identity, self.into_facts())
    }

    fn into_facts(mut self) -> OriginFacts {
        self.references.sort();
        self.references.dedup();
        let contains_encrypted_source = match self.encryption {
            SourceEncryptionState::Encrypted => Some(true),
            SourceEncryptionState::Unencrypted if self.complete => Some(false),
            _ => None,
        };
        OriginFacts {
            contains_encrypted_source,
            references: self.references,
            source_set_complete: self.complete,
        }
    }
}

impl FieldOrigin {
    pub(crate) fn new<'a>(
        identity: Arc<()>,
        uses: impl IntoIterator<Item = &'a SourceUse>,
        complete: bool,
    ) -> Self {
        Self::from_descriptions(
            identity,
            uses.into_iter().map(SourceUse::description),
            complete,
        )
    }

    pub(crate) fn from_descriptions<'a>(
        identity: Arc<()>,
        uses: impl IntoIterator<Item = &'a SourceUseDescription>,
        complete: bool,
    ) -> Self {
        Self::from_facts(identity, OriginFacts::measure(uses, complete))
    }

    fn from_facts(identity: Arc<()>, facts: OriginFacts) -> Self {
        Self {
            identity: Some(identity),
            fields: OriginFields::from_refs(
                facts.contains_encrypted_source,
                facts.references,
                facts.source_set_complete,
            ),
        }
    }

    pub(crate) fn with_shared<'a>(
        shared: &SharedFieldOrigin,
        uses: impl IntoIterator<Item = &'a SourceUse>,
        complete: bool,
    ) -> Self {
        Self {
            identity: Some(Arc::clone(&shared.identity)),
            fields: shared.fields_with(OriginFacts::measure(
                uses.into_iter().map(SourceUse::description),
                complete,
            )),
        }
    }

    pub(crate) fn belongs_to(&self, identity: &Arc<()>) -> bool {
        self.identity
            .as_ref()
            .is_some_and(|original| Arc::ptr_eq(original, identity))
    }

    /// Returns true for any proved encrypted contributor, false only for a complete unencrypted set.
    #[must_use]
    pub fn contains_encrypted_source(&self) -> Option<bool> {
        self.fields.contains_encrypted_source
    }

    /// Returns the exact contributing use references, excluding attempts and untouched targets.
    #[must_use]
    pub fn source_uses_refs(&self) -> &[SourceUseRef] {
        self.fields.source_uses_refs.as_slice()
    }

    pub(crate) fn iter_source_use_refs(&self) -> impl ExactSizeIterator<Item = &SourceUseRef> {
        self.fields.source_uses_refs.iter()
    }

    /// Returns completeness of this contribution set, independently of source or export health.
    #[must_use]
    pub fn source_set_complete(&self) -> bool {
        self.fields.source_set_complete
    }
}

impl PartialEq for FieldOrigin {
    fn eq(&self, other: &Self) -> bool {
        self.fields == other.fields
    }
}

impl Eq for FieldOrigin {}

impl Serialize for FieldOrigin {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.fields.as_ref().serialize(serializer)
    }
}

impl JsonSchema for FieldOrigin {
    fn schema_name() -> Cow<'static, str> {
        "FieldOrigin".into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        OriginFields::json_schema(generator)
    }
}

impl FieldOrigin {
    pub(crate) fn from_wire(
        contains_encrypted_source: Option<bool>,
        source_uses_refs: Vec<SourceUseRef>,
        source_set_complete: bool,
    ) -> Result<Self, &'static str> {
        if contains_encrypted_source == Some(false) && !source_set_complete {
            return Err("unencrypted field origin requires complete contributors");
        }
        let empty_marker = if source_set_complete {
            Some(false)
        } else {
            None
        };
        if source_uses_refs.is_empty() && contains_encrypted_source != empty_marker {
            return Err("empty contributor origin has an inconsistent marker");
        }
        Ok(Self {
            identity: None,
            fields: OriginFields::from_refs(
                contains_encrypted_source,
                source_uses_refs,
                source_set_complete,
            ),
        })
    }
}
