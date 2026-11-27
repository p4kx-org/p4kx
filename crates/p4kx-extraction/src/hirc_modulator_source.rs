// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::io::{Read, Seek};

use p4kx_audio::{BankHircModulator, BankHircObject, BankNodePropertyKind, BankObservedHeader};
use schemars::JsonSchema;
use serde::ser::Error as _;
use serde::{Serialize, Serializer};

use crate::source_inventory::SourceInventory;

use super::super::super::node_property::{PropertyList, PropertyWords};
use super::{Coverage, InitialRtpc, MetadataDocument, ObjectOrigins, RtpcSchema};

pub(in crate::hirc_view) type ModulatorSourceSchema =
    MetadataDocument<ModulatorTag, ObservedModulation, RtpcSchema, SourceInventory>;

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(in crate::hirc_view) enum ModulatorTag {
    Modulator,
}

#[derive(Default, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(in crate::hirc_view) struct ObservedModulation {
    #[serde(skip_serializing_if = "Option::is_none")]
    properties: Option<PropertyList<PropertyWords>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ranged_modifiers: Option<PropertyList<PropertyWords>>,
}

pub(in crate::hirc_view) fn serialize<R: Read + Seek, S: Serializer>(
    serializer: S,
    origins: &ObjectOrigins,
    header: &BankObservedHeader,
    mut reader: BankHircModulator<'_, R>,
) -> Result<S::Ok, S::Error> {
    let coverage = Coverage::checked(
        origins,
        header,
        reader.object(),
        reader.format_error(),
        reader.checked_range(),
        reader.unprojected_range(),
    )
    .map_err(S::Error::custom)?;
    let observed = ObservedModulation {
        properties: PropertyList::read(
            origins,
            reader.properties(BankNodePropertyKind::Properties),
            |row| PropertyWords::read(origins, &row),
        )?,
        ranged_modifiers: PropertyList::read(
            origins,
            reader.properties(BankNodePropertyKind::RangedModifiers),
            |row| PropertyWords::read(origins, &row),
        )?,
    };
    let has_rtpc = reader.fields().rtpc.is_some();
    let reader = RefCell::new(reader);
    MetadataDocument {
        kind: ModulatorTag::Modulator,
        coverage,
        observed,
        initial_rtpc: has_rtpc.then_some(InitialRtpc::Modulator(&reader, origins)),
        source_inventory: origins,
    }
    .serialize(serializer)
}

pub(in crate::hirc_view) fn unsupported<S: Serializer>(
    serializer: S,
    origins: &ObjectOrigins,
    header: &BankObservedHeader,
    object: &BankHircObject,
) -> Result<S::Ok, S::Error> {
    MetadataDocument {
        kind: ModulatorTag::Modulator,
        coverage: Coverage::unsupported(origins, header, object).map_err(S::Error::custom)?,
        observed: ObservedModulation::default(),
        initial_rtpc: None::<RtpcSchema>,
        source_inventory: origins,
    }
    .serialize(serializer)
}
