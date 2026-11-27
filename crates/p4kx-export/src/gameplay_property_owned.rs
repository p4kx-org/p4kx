// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use p4kx_extraction::SourcedValue;

use super::contract::{
    GameplayProperty, GameplayPropertyDefinition, GameplayPropertyFields,
    PropertyNameOverrideConditionPayload, PropertyNameOverrideConditionSource,
    PropertyNameOverrideFields, PropertyNameOverrideSource,
};
use super::transformation_contract::{
    DisplayTransformationNodeSource, DisplayTransformationSource,
};

impl From<GameplayPropertyDefinition<'_>> for GameplayProperty {
    fn from(row: GameplayPropertyDefinition<'_>) -> Self {
        Self {
            source: row.source.into(),
            fields: GameplayPropertyFields {
                property_name: row.fields.property_name,
                display_transformation: row.fields.display_transformation.map(own_transformation),
                unit_format: row.fields.unit_format,
                name_overrides: row.fields.name_overrides.map(|slots| {
                    slots
                        .into_iter()
                        .map(|slot| slot.map(own_override))
                        .collect()
                }),
            },
        }
    }
}

fn own_transformation(
    source: DisplayTransformationSource<'_>,
) -> DisplayTransformationSource<'static> {
    DisplayTransformationSource {
        root: source.root,
        nodes: source
            .nodes
            .into_iter()
            .map(|node| DisplayTransformationNodeSource {
                source_type: node.source_type.map(own_label),
                payload: node.payload,
            })
            .collect(),
    }
}

fn own_override(source: PropertyNameOverrideSource<'_>) -> PropertyNameOverrideSource<'static> {
    PropertyNameOverrideSource {
        source_type: source.source_type.map(own_label),
        payload: source.payload.map(|fields| PropertyNameOverrideFields {
            property_name: fields.property_name,
            condition: fields.condition.map(own_condition),
        }),
    }
}

fn own_condition(
    source: PropertyNameOverrideConditionSource<'_>,
) -> PropertyNameOverrideConditionSource<'static> {
    PropertyNameOverrideConditionSource {
        source_type: source.source_type.map(own_label),
        payload: source.payload.map(|payload| match payload {
            PropertyNameOverrideConditionPayload::EmptyBase => {
                PropertyNameOverrideConditionPayload::EmptyBase
            }
            PropertyNameOverrideConditionPayload::ItemType {
                match_item_types,
                match_sub_types,
            } => PropertyNameOverrideConditionPayload::ItemType {
                match_item_types: match_item_types.map(own_labels),
                match_sub_types: match_sub_types.map(own_labels),
            },
        }),
    }
}

fn own_label(label: Cow<'_, str>) -> Cow<'static, str> {
    Cow::Owned(label.into_owned())
}

fn own_labels(slots: Vec<SourcedValue<Cow<'_, str>>>) -> Vec<SourcedValue<Cow<'static, str>>> {
    slots.into_iter().map(|slot| slot.map(own_label)).collect()
}
