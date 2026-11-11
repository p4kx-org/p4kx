// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::authored_nodes::AuthoredNodes;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

use super::super::{read_child, read_children};

/// The original mode and ordered sequence entries.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct SequenceAction<'source> {
    mode: SourcedValue<&'source str>,
    sequence_entries: SourcedValue<Vec<SourcedValue<SequenceActionEntry<'source>>>>,
    authored: SourceArenaOutput<'source>,
}

impl<'source> SequenceAction<'source> {
    pub(super) fn read<'reader, 'scope, 'read>(
        source: Observed<'reader, 'scope, 'read, 'source>,
        pending: &mut AuthoredNodes<'reader, 'scope, 'read, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let mode = source.borrowed_enum_to("mode", "ESequenceMode", notes)?;
        let sequence_entries = source.class_array_to(
            "sequenceEntries",
            "SWeaponSequenceEntryParams",
            notes,
            |entry, notes| SequenceActionEntry::read(entry, pending, notes),
        )?;
        let authored = source.authored_members_except([
            ("mode", mode.state()),
            ("sequenceEntries", sequence_entries.state()),
        ])?;
        Ok(Self {
            mode,
            sequence_entries,
            authored,
        })
    }
}

/// The exact timing and repetitions owned by an original sequence entry.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct SequenceActionEntry<'source> {
    delay: SourcedValue<ResourceFloat32>,
    unit: SourcedValue<&'source str>,
    repetitions: SourcedValue<i32>,
    weapon_action: SourcedValue<u32>,
    authored: SourceArenaOutput<'source>,
}

impl<'source> SequenceActionEntry<'source> {
    fn read<'reader, 'scope, 'read>(
        source: Observed<'reader, 'scope, 'read, 'source>,
        pending: &mut AuthoredNodes<'reader, 'scope, 'read, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let delay = source.single("delay", notes)?;
        let unit = source.borrowed_enum_to("unit", "EDelayUnit", notes)?;
        let repetitions = source.int32("repetitions", notes)?;
        let weapon_action = read_child(source, "weaponAction", pending, notes)?;
        let authored = source.authored_members_except([
            ("delay", delay.state()),
            ("unit", unit.state()),
            ("repetitions", repetitions.state()),
            ("weaponAction", weapon_action.state()),
        ])?;
        Ok(Self {
            delay,
            unit,
            repetitions,
            weapon_action,
            authored,
        })
    }
}

/// The ordered original slots of a parallel action.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ParallelAction<'source> {
    weapon_actions: SourcedValue<Vec<SourcedValue<u32>>>,
    authored: SourceArenaOutput<'source>,
}

impl<'source> ParallelAction<'source> {
    pub(super) fn read<'reader, 'scope, 'read>(
        source: Observed<'reader, 'scope, 'read, 'source>,
        pending: &mut AuthoredNodes<'reader, 'scope, 'read, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let weapon_actions = read_children(source, "weaponActions", pending, notes)?;
        let authored =
            source.authored_members_except([("weaponActions", weapon_actions.state())])?;
        Ok(Self {
            weapon_actions,
            authored,
        })
    }
}
