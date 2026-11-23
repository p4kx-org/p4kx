// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::sync::Arc;

use p4kx_extraction::{
    FieldOrigin, SourceUseRef, SourceUses, SourceValue, SourceValueState, SourcedValue,
};
use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Serialize, Serializer};

use crate::schema_fields;
use crate::variant_target::VariantTarget;
use crate::variant_use_sets::VariantSourceUseSet;

#[derive(Debug, Clone)]
pub(crate) struct VariantDecision {
    expanded: Arc<SourcedValue<VariantTarget>>,
    group: Option<SharedUses>,
    failed_source_uses: Vec<SourceUseRef>,
}

#[derive(Debug, Clone)]
struct SharedUses {
    set: Arc<VariantSourceUseSet>,
    local: Arc<[SourceUseRef]>,
}

impl VariantDecision {
    pub(crate) fn unresolved(uses: &SourceUses) -> Self {
        Self::local(uses.value(SourceValue::Unreadable, &[], false))
    }

    pub(crate) fn local(expanded: SourcedValue<VariantTarget>) -> Self {
        Self {
            expanded: Arc::new(expanded),
            group: None,
            failed_source_uses: Vec::new(),
        }
    }

    pub(crate) fn shared(
        expanded: SourcedValue<VariantTarget>,
        set: &Arc<VariantSourceUseSet>,
    ) -> Self {
        let basis = set.source_uses_refs();
        let local = expanded
            .origin()
            .source_uses_refs()
            .iter()
            .filter(|reference| basis.binary_search(reference).is_err())
            .cloned()
            .collect();
        Self {
            expanded: Arc::new(expanded),
            group: Some(SharedUses {
                set: Arc::clone(set),
                local,
            }),
            failed_source_uses: Vec::new(),
        }
    }

    pub(crate) fn with_failed_source_uses(mut self, failed_source_uses: Vec<SourceUseRef>) -> Self {
        self.failed_source_uses = failed_source_uses;
        self
    }

    pub(crate) fn failed_source_uses(&self) -> &[SourceUseRef] {
        &self.failed_source_uses
    }

    pub(crate) fn expanded(&self) -> &SourcedValue<VariantTarget> {
        &self.expanded
    }

    pub(crate) fn origin(&self) -> &FieldOrigin {
        self.expanded.origin()
    }

    pub(crate) fn value(&self) -> Option<&VariantTarget> {
        self.expanded.value()
    }

    pub(crate) fn shared_uses(&self) -> Option<&Arc<VariantSourceUseSet>> {
        self.group.as_ref().map(|shared| &shared.set)
    }

    pub(crate) fn source_use_set_ref(&self) -> Option<u32> {
        self.shared_uses().map(|set| set.id())
    }

    pub(crate) fn local_uses(&self) -> &[SourceUseRef] {
        self.group.as_ref().map_or_else(
            || self.expanded.origin().source_uses_refs(),
            |shared| shared.local.as_ref(),
        )
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = correlate_decision)]
struct VariantDecisionWire<'source> {
    #[schemars(extend(
        "enum" = ["present", "absent", "unreadable"],
        "x-p4kx-vocabularyClass" = "control"
    ))]
    state: SourceValueState,
    value: Option<&'source VariantTarget>,
    contains_encrypted_source: Option<bool>,
    #[schemars(extend("uniqueItems" = true))]
    source_uses_refs: &'source [SourceUseRef],
    source_set_complete: bool,
    source_use_set_ref: Option<u32>,
    #[schemars(extend("uniqueItems" = true))]
    failed_source_use_refs: &'source [SourceUseRef],
}

fn correlate_decision(schema: &mut Schema) {
    schema_fields::every_field_required(schema);
    schema.insert(
        "allOf".into(),
        serde_json::json!([
            {
                "if": {"properties": {"state": {"const": "present"}}},
                "then": {"properties": {
                    "value": {"type": "object"},
                    "sourceUseSetRef": {"type": "integer"}
                }},
                "else": {"properties": {"value": {"type": "null"}}}
            },
            {
                "if": {"properties": {"state": {"const": "unreadable"}}},
                "then": {"properties": {
                    "sourceSetComplete": {"const": false},
                    "containsEncryptedSource": {"enum": [true, null], "x-p4kx-vocabularyClass": "control"}
                }},
                "else": {"properties": {"sourceSetComplete": {"const": true}}}
            },
            {
                "if": {"properties": {
                    "sourceUseSetRef": {"type": "null"},
                    "sourceUsesRefs": {"maxItems": 0}
                }},
                "then": {
                    "if": {"properties": {"sourceSetComplete": {"const": true}}},
                    "then": {"properties": {"containsEncryptedSource": {"const": false}}},
                    "else": {"properties": {"containsEncryptedSource": {"type": "null"}}}
                }
            }
        ]),
    );
}

impl Serialize for VariantDecision {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let origin = self.origin();
        VariantDecisionWire {
            state: self.expanded.state(),
            value: self.expanded.value(),
            contains_encrypted_source: origin.contains_encrypted_source(),
            source_uses_refs: self.local_uses(),
            source_set_complete: origin.source_set_complete(),
            source_use_set_ref: self.source_use_set_ref(),
            failed_source_use_refs: self.failed_source_uses(),
        }
        .serialize(serializer)
    }
}

impl JsonSchema for VariantDecision {
    fn schema_name() -> Cow<'static, str> {
        "VariantDecision".into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        VariantDecisionWire::json_schema(generator)
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use p4kx_extraction::{
        InputScope, SourceInput, SourceLocation, SourceUseRole, SourceUses, SourceValue,
    };
    use serde_json::json;

    use super::VariantDecision;

    #[test]
    fn a_failed_variant_operand_keeps_a_receipt_outside_the_contributors()
    -> Result<(), Box<dyn Error>> {
        let uses = SourceUses::new();
        let failed = uses
            .observe(SourceInput::Unknown, InputScope::Payload)?
            .at(
                SourceLocation::Metadata {
                    field: "fixtureOperand".into(),
                },
                SourceUseRole::Attempted,
            )?;
        let receipt = failed.reference().clone();
        let expanded = uses.value(SourceValue::Unreadable, &[failed], false);
        let decision = VariantDecision::local(expanded).with_failed_source_uses(vec![receipt]);
        let body = serde_json::to_value(&decision)?;
        assert_eq!(
            body["failedSourceUseRefs"],
            json!(decision.failed_source_uses()),
            "the attempted read is an explicit wire receipt"
        );
        assert_eq!(
            body["sourceUsesRefs"],
            json!([]),
            "the failed read is outside the contributing source uses"
        );
        assert_eq!(
            body["sourceSetComplete"],
            json!(false),
            "the attempted read does not establish a complete decision"
        );
        Ok(())
    }
}
