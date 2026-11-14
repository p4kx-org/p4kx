// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::io::{Read, Seek};

use p4kx_audio::{
    BankHircField, BankInlineStateRow, BankInlineStateRows, StateProperties, StateProperty,
};
use schemars::JsonSchema;
use serde::ser::{Error as _, SerializeSeq};
use serde::{Serialize, Serializer};

use crate::source_evidence_error::SourceEvidenceError;
use crate::source_value::SourcedValue;

use super::super::StateRange;
use super::ObjectOrigins;
use super::reader::NodeReader;

pub(super) struct InlineStateGroup<'view, 'source, 'input, R> {
    pub(super) origins: &'view ObjectOrigins,
    pub(super) node: &'view RefCell<NodeReader<'source, 'input, R>>,
}

impl<R: Read + Seek> Serialize for InlineStateGroup<'_, '_, '_, R> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut node = self.node.try_borrow_mut().map_err(S::Error::custom)?;
        let rows = node
            .states()
            .ok_or_else(|| S::Error::custom("the checked inline State region is absent"))?;
        InlineState::read(self.origins, rows)
            .map_err(S::Error::custom)?
            .serialize(serializer)
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(in crate::hirc_view) struct InlineState<Rows> {
    range: SourcedValue<StateRange>,
    #[schemars(with = "Vec<InlineStateRow<Vec<InlineStateProperty>>>")]
    rows: Rows,
}

pub(super) type InlineStateSchema = InlineState<Vec<InlineStateRow<Vec<InlineStateProperty>>>>;

impl<'input, R> InlineState<InlineStateRows<'_, 'input, R>> {
    pub(super) fn read<'view>(
        origins: &'view ObjectOrigins,
        rows: BankInlineStateRows<'input, R>,
    ) -> Result<InlineState<InlineStateRows<'view, 'input, R>>, SourceEvidenceError> {
        Ok(InlineState {
            range: origins.range(rows.range(), "nodeInlineStateExtent")?,
            rows: InlineStateRows {
                origins,
                rows: RefCell::new(rows),
            },
        })
    }
}

pub(super) struct InlineStateRows<'view, 'input, R> {
    origins: &'view ObjectOrigins,
    rows: RefCell<BankInlineStateRows<'input, R>>,
}

impl<R: Read + Seek> Serialize for InlineStateRows<'_, '_, R> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut rows = self.rows.try_borrow_mut().map_err(S::Error::custom)?;
        let mut sequence = serializer.serialize_seq(None)?;
        for row in &mut *rows {
            sequence.serialize_element(&InlineStateRow::read(
                self.origins,
                row.map_err(S::Error::custom)?,
            )?)?;
        }
        sequence.end()
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub(in crate::hirc_view) enum InlineStateRow<Properties> {
    BindingCount {
        count: SourcedValue<u32>,
    },
    Binding {
        ordinal: SourcedValue<u32>,
        property_id: SourcedValue<u32>,
        accumulation: SourcedValue<u8>,
        in_db: SourcedValue<u8>,
    },
    GroupCount {
        count: SourcedValue<u32>,
    },
    Group {
        ordinal: SourcedValue<u32>,
        group_id: SourcedValue<u32>,
        synchronization: SourcedValue<u8>,
        state_count: SourcedValue<u32>,
    },
    State {
        group_ordinal: SourcedValue<u32>,
        ordinal: SourcedValue<u32>,
        state_id: SourcedValue<u32>,
        property_count: SourcedValue<u16>,
        #[schemars(with = "Vec<InlineStateProperty>")]
        properties: Properties,
    },
}

impl<'view> InlineStateRow<InlineStateProperties<'view>> {
    fn read<E: serde::ser::Error>(
        origins: &'view ObjectOrigins,
        row: BankInlineStateRow,
    ) -> Result<Self, E> {
        Ok(match row {
            BankInlineStateRow::BindingCount(count) => Self::BindingCount {
                count: origins
                    .field(&count, "nodeInlineStateBindingCount")
                    .map_err(E::custom)?,
            },
            BankInlineStateRow::Binding { ordinal, fields } => Self::Binding {
                ordinal: origins
                    .derived(
                        ordinal,
                        &fields.property_id.range,
                        "nodeInlineStateBindingPosition",
                    )
                    .map_err(E::custom)?,
                property_id: origins
                    .field(&fields.property_id, "nodeInlineStateBindingPropertyId")
                    .map_err(E::custom)?,
                accumulation: origins
                    .field(&fields.accumulation, "nodeInlineStateAccumulation")
                    .map_err(E::custom)?,
                in_db: origins
                    .field(&fields.in_db, "nodeInlineStateInDb")
                    .map_err(E::custom)?,
            },
            BankInlineStateRow::GroupCount(count) => Self::GroupCount {
                count: origins
                    .field(&count, "nodeInlineStateGroupCount")
                    .map_err(E::custom)?,
            },
            BankInlineStateRow::Group { ordinal, fields } => Self::Group {
                ordinal: origins
                    .derived(
                        ordinal,
                        &fields.group_id.range,
                        "nodeInlineStateGroupPosition",
                    )
                    .map_err(E::custom)?,
                group_id: origins
                    .field(&fields.group_id, "nodeInlineStateGroupId")
                    .map_err(E::custom)?,
                synchronization: origins
                    .field(&fields.synchronization, "nodeInlineStateSynchronization")
                    .map_err(E::custom)?,
                state_count: origins
                    .field(&fields.state_count, "nodeInlineStateMemberCount")
                    .map_err(E::custom)?,
            },
            BankInlineStateRow::State {
                group,
                ordinal,
                state_id,
                properties,
            } => {
                return Self::member(origins, group, ordinal, &state_id, properties)
                    .map_err(E::custom);
            }
            _ => return Err(E::custom("unexpected original inline State row")),
        })
    }

    fn member(
        origins: &'view ObjectOrigins,
        group: u32,
        ordinal: u32,
        state_id: &BankHircField<u32>,
        properties: StateProperties,
    ) -> Result<Self, SourceEvidenceError> {
        Ok(Self::State {
            group_ordinal: origins.derived(
                group,
                &state_id.range,
                "nodeInlineStateOwningGroupPosition",
            )?,
            ordinal: origins.derived(ordinal, &state_id.range, "nodeInlineStateMemberPosition")?,
            state_id: origins.field(state_id, "nodeInlineStateMemberId")?,
            property_count: origins.field(properties.count(), "nodeInlineStatePropertyCount")?,
            properties: InlineStateProperties {
                origins,
                properties,
            },
        })
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(in crate::hirc_view) struct InlineStateProperty {
    property_id: SourcedValue<u16>,
    value_bits: SourcedValue<u32>,
}

impl InlineStateProperty {
    fn read(
        origins: &ObjectOrigins,
        original: &StateProperty,
    ) -> Result<Self, SourceEvidenceError> {
        Ok(Self {
            property_id: origins.field(&original.id, "nodeInlineStatePropertyId")?,
            value_bits: origins.field(&original.value_bits, "nodeInlineStatePropertyValueBits")?,
        })
    }
}

struct InlineStateProperties<'view> {
    origins: &'view ObjectOrigins,
    properties: StateProperties,
}

impl Serialize for InlineStateProperties<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut sequence =
            serializer.serialize_seq(Some(usize::from(self.properties.count().value)))?;
        for property in self.properties.entries() {
            let property =
                InlineStateProperty::read(self.origins, &property).map_err(S::Error::custom)?;
            sequence.serialize_element(&property)?;
        }
        sequence.end()
    }
}
