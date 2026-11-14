// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use anyhow::{Result, ensure};
use p4kx_extraction::{
    DataType, Guid, Record, RecordTree, SourceLocation, SourceScope, SourceUse, SourceUseRole,
    TAG_DATABASE_TYPE, TagPathIncoming, Value, ValueError,
};

use super::Proof;

#[derive(Clone, Copy)]
pub(super) enum HeaderField {
    Guid,
    SourceType,
}

impl HeaderField {
    const fn name(self) -> &'static str {
        match self {
            Self::Guid => "guid",
            Self::SourceType => "sourceType",
        }
    }
}

pub(super) fn header(
    scope: &SourceScope,
    record_index: u32,
    field: HeaderField,
    role: SourceUseRole,
) -> Result<SourceUse> {
    Ok(scope.at(
        SourceLocation::Subresource {
            namespace: "dataforgeRecordHeader".to_owned(),
            indices: vec![u64::from(record_index)],
            field: field.name().to_owned(),
        },
        role,
    )?)
}

pub(super) fn body(
    scope: &SourceScope,
    record_index: u32,
    field: &str,
    role: SourceUseRole,
) -> Result<SourceUse> {
    Ok(scope.at(
        SourceLocation::DataForge {
            record_index,
            field: field.to_owned(),
        },
        role,
    )?)
}

#[derive(Clone, Copy)]
enum Read {
    Value,
    Missing,
    Fallback,
    Unreadable,
}

impl Read {
    fn observe(
        self,
        scope: &SourceScope,
        record_index: u32,
        field: &str,
        value_role: SourceUseRole,
    ) -> Result<Proof> {
        let used = match self {
            Self::Value => body(scope, record_index, field, value_role)?,
            Self::Missing => scope.at(
                SourceLocation::Subresource {
                    namespace: "dataforgeRecordDeclarations".to_owned(),
                    indices: vec![u64::from(record_index)],
                    field: "$fields".to_owned(),
                },
                SourceUseRole::Selector,
            )?,
            Self::Fallback => body(scope, record_index, field, SourceUseRole::Selector)?,
            Self::Unreadable => body(scope, record_index, field, SourceUseRole::Attempted)?,
        };
        Ok(Proof::one(used, !matches!(self, Self::Unreadable)))
    }
}

#[derive(Clone, Copy)]
enum RelationSlot {
    Linked {
        guid: Guid,
        target_index: u32,
        storage: DataType,
    },
    Dangling(Guid),
    Null,
    Malformed,
    Unreadable,
}

impl RelationSlot {
    fn read(value: &Value<'_>, storage: DataType, record: &Record<'_>) -> Result<Self> {
        Ok(match value {
            Value::Reference(reference) => {
                let target = reference.original_target();
                ensure!(
                    target.shares_source_with(record),
                    "foreign Tag relation target"
                );
                Self::Linked {
                    guid: target.record_guid(),
                    target_index: target.record_index(),
                    storage,
                }
            }
            Value::Error(ValueError::DanglingReference { guid }) => Self::Dangling(*guid),
            Value::Null => Self::Null,
            Value::Error(_) => Self::Unreadable,
            _ => Self::Malformed,
        })
    }
}

pub(super) struct Canonical {
    index: u32,
    name: Read,
    legacy: Read,
    sentinel: bool,
    pub(super) relations: &'static str,
    relation_read: Read,
    slots: Vec<RelationSlot>,
}

impl Canonical {
    pub(super) fn read(record: &Record<'_>, tree: &RecordTree<'_>) -> Result<Self> {
        ensure!(tree.is_from(record), "foreign canonical Tag tree");
        let name = match tree.field("tagName") {
            Some(Value::String(_)) => Read::Value,
            Some(Value::Error(_)) => Read::Unreadable,
            Some(_) => Read::Fallback,
            None => Read::Missing,
        };
        let legacy = match tree.field("legacyGUID") {
            Some(Value::UInt32(_)) => Read::Value,
            Some(Value::Error(_)) => Read::Unreadable,
            Some(_) => Read::Fallback,
            None => Read::Missing,
        };
        let sentinel = matches!(tree.field("legacyGUID"), Some(Value::UInt32(u32::MAX)));
        let relations = if tree.type_name() == TAG_DATABASE_TYPE {
            "tags"
        } else {
            "children"
        };
        let mut slots = Vec::new();
        let relation_read = match tree.fields().iter().find(|field| field.name() == relations) {
            Some(field) => match field.value() {
                Value::Array(values) => {
                    let storage = field.storage();
                    ensure!(storage.is_array(), "invalid Tag relation storage");
                    slots.reserve(values.len());
                    for value in values {
                        slots.push(RelationSlot::read(value, storage.data_type(), record)?);
                    }
                    Read::Value
                }
                Value::Error(_) => Read::Unreadable,
                _ => Read::Fallback,
            },
            None => Read::Missing,
        };
        Ok(Self {
            index: record.index(),
            name,
            legacy,
            sentinel,
            relations,
            relation_read,
            slots,
        })
    }

    pub(super) fn name(&self, scope: &SourceScope, role: SourceUseRole) -> Result<Proof> {
        self.name.observe(scope, self.index, "tagName", role)
    }

    pub(super) fn legacy(&self, scope: &SourceScope) -> Result<Proof> {
        let role = if self.sentinel {
            SourceUseRole::Conversion
        } else {
            SourceUseRole::Payload
        };
        self.legacy.observe(scope, self.index, "legacyGUID", role)
    }

    pub(super) fn array(&self, scope: &SourceScope) -> Result<Proof> {
        let mut proof = self.relation_read.observe(
            scope,
            self.index,
            self.relations,
            SourceUseRole::Selector,
        )?;
        proof.complete &= !self
            .slots
            .iter()
            .any(|slot| matches!(slot, RelationSlot::Unreadable));
        Ok(proof)
    }

    pub(super) fn edge(
        &self,
        scope: &SourceScope,
        incoming: TagPathIncoming<'_, '_>,
        target: Option<Guid>,
    ) -> Result<Proof> {
        ensure!(
            incoming.record().index() == self.index && incoming.kind().field() == self.relations,
            "invalid Tag relation owner"
        );
        let slot = self
            .slots
            .get(incoming.ordinal())
            .ok_or_else(|| anyhow::anyhow!("missing original Tag relation slot"))?;
        let field = format!("{}[{}]", self.relations, incoming.ordinal());
        let role = match *slot {
            RelationSlot::Linked { guid, .. } | RelationSlot::Dangling(guid) => {
                ensure!(target == Some(guid), "invalid Tag relation target");
                SourceUseRole::Selector
            }
            RelationSlot::Malformed | RelationSlot::Unreadable => {
                ensure!(target.is_none(), "invalid malformed Tag relation target");
                if matches!(slot, RelationSlot::Unreadable) {
                    SourceUseRole::Attempted
                } else {
                    SourceUseRole::Selector
                }
            }
            RelationSlot::Null => anyhow::bail!("a null Tag relation was selected"),
        };
        let used = body(scope, self.index, &field, role)?;
        let mut proof = Proof::one(used, role != SourceUseRole::Attempted);
        if let RelationSlot::Linked {
            target_index,
            storage,
            ..
        } = *slot
            && storage != DataType::Reference
        {
            proof.uses.push(header(
                scope,
                target_index,
                HeaderField::Guid,
                SourceUseRole::Conversion,
            )?);
        }
        Ok(proof)
    }
}
