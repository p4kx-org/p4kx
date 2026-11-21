// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_audio::{BankHircObject, BankObservedHeader, BankStateRead, StateProperties};
use schemars::JsonSchema;
use serde::ser::{Error as _, SerializeSeq};
use serde::{Serialize, Serializer};

use crate::hirc_view::StateRange;
use crate::source_inventory::SourceInventory;
use crate::source_value::SourcedValue;

use super::origins::ObjectOrigins;

pub(super) type StateSourceSchema = StateDocument<Vec<StateProperty>, SourceInventory>;

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "StateObjectSourceFields")]
pub(in crate::hirc_view::source_fields) struct StateDocument<Rows, Inventory> {
    kind: StateKind,
    layout: SourcedValue<Layout>,
    version: SourcedValue<u32>,
    raw_body_range: SourcedValue<StateRange>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<SourcedValue<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bundle: Option<StateBundle<Rows>>,
    #[schemars(with = "SourceInventory")]
    source_inventory: Inventory,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
enum StateKind {
    State,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
enum Layout {
    Decoded,
    Malformed,
    Unsupported,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct StateBundle<Rows> {
    property_count: SourcedValue<u16>,
    consumed_range: SourcedValue<StateRange>,
    trailing_range: SourcedValue<StateRange>,
    #[schemars(with = "Vec<StateProperty>")]
    properties: Rows,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(in crate::hirc_view::source_fields) struct StateProperty {
    id: SourcedValue<u16>,
    value_bits: SourcedValue<u32>,
    value: SourcedValue<Option<f32>>,
}

pub(super) fn serialize<S: Serializer>(
    serializer: S,
    origins: &ObjectOrigins,
    header: &BankObservedHeader,
    object: &BankHircObject,
    status: Option<&BankStateRead>,
) -> Result<S::Ok, S::Error> {
    let (layout, error, bundle) = match status {
        Some(BankStateRead::Decoded(state)) => {
            let properties = &state.properties;
            let bundle = StateBundle {
                property_count: origins
                    .field(properties.count(), "statePropertyCount")
                    .map_err(S::Error::custom)?,
                consumed_range: origins
                    .range(&properties.consumed_range(), "statePropertyExtent")
                    .map_err(S::Error::custom)?,
                trailing_range: origins
                    .range(&state.trailing_range, "stateTrailingExtent")
                    .map_err(S::Error::custom)?,
                properties: StateRows {
                    properties,
                    origins,
                },
            };
            (Layout::Decoded, None, Some(bundle))
        }
        Some(BankStateRead::Malformed(error)) => (Layout::Malformed, Some(error), None),
        Some(BankStateRead::Unsupported { .. }) | None => (Layout::Unsupported, None, None),
        _ => return Err(S::Error::custom("unexpected standalone State disposition")),
    };
    StateDocument {
        kind: StateKind::State,
        layout: origins
            .coverage(layout, header, object)
            .map_err(S::Error::custom)?,
        version: origins.version(header).map_err(S::Error::custom)?,
        raw_body_range: origins
            .range(
                &(object.body_offset()..object.body_end_exclusive()),
                "stateRawBodyExtent",
            )
            .map_err(S::Error::custom)?,
        error: error
            .map(|error| origins.coverage(error.to_string(), header, object))
            .transpose()
            .map_err(S::Error::custom)?,
        bundle,
        source_inventory: origins,
    }
    .serialize(serializer)
}

struct StateRows<'a> {
    properties: &'a StateProperties,
    origins: &'a ObjectOrigins,
}

impl Serialize for StateRows<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut rows =
            serializer.serialize_seq(Some(usize::from(self.properties.count().value)))?;
        for property in self.properties.entries() {
            let original = property.value();
            rows.serialize_element(&StateProperty {
                id: self
                    .origins
                    .field(&property.id, "statePropertyId")
                    .map_err(S::Error::custom)?,
                value_bits: self
                    .origins
                    .field(&property.value_bits, "statePropertyBits")
                    .map_err(S::Error::custom)?,
                value: self
                    .origins
                    .derived(
                        original.is_finite().then_some(original),
                        &property.value_bits.range,
                        "statePropertyNumber",
                    )
                    .map_err(S::Error::custom)?,
            })?;
        }
        rows.end()
    }
}
