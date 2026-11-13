// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, StructValue, Value as Original};
use serde_json::Value;

use super::{TestResult, single};
use crate::authored_definition_witness::{verify_array, verify_link, verify_state};
use crate::real_archive_auto_spawns::{original::Locations, provenance::Origins};
use crate::real_archive_item_components::{cell, owned, require_state};
use crate::real_archive_item_durability as proof;

pub(super) struct Witness<'catalog, 'source> {
    pub(super) owner: u32,
    pub(super) locations: Locations<'source>,
    pub(super) origins: &'catalog Origins<'catalog>,
}

impl<'source> Witness<'_, 'source> {
    pub(super) fn compare(
        &self,
        forge: &DataForge<'_>,
        source: &StructValue<'source>,
        row: &Value,
    ) -> TestResult<usize> {
        let mut checked = 0;
        for name in source_members(source.type_name()) {
            if let Some(value) = source.field(name)
                && !matches!(value, Original::Error(_))
            {
                let part = if matches!(value, Original::Struct(_)) {
                    "$type"
                } else {
                    "$value"
                };
                self.occurrence(value, cell(row, wire_name(name))?, part)?;
                checked += 1;
            }
        }
        match source.type_name() {
            "SCItemFuelTankParams" => compare_tank(source, row)?,
            "ResourceContainer" => compare_policy(forge, source, row)?,
            "SCItemFuelIntakeParams" => {}
            _ => return Err("unsupported fuel witness family".into()),
        }
        Ok(checked)
    }

    pub(super) fn occurrence(
        &self,
        value: &Original<'source>,
        row: &Value,
        field: &str,
    ) -> TestResult {
        self.origins
            .occurrence(row, self.owner, self.locations.ordinal(value)?, field)
    }
}

fn source_members(family: &str) -> &'static [&'static str] {
    match family {
        "SCItemFuelTankParams" => &[
            "hydrogenMaxFlowMultiplier",
            "quantumMaxFlowMultiplier",
            "openState",
            "closedState",
            "pumpingState",
            "fuelFlowLoopStartAudioTrigger",
            "fuelFlowLoopStopAudioTrigger",
            "fuelFillDrainRateAudioRtpc",
            "fuelFillLevelAudioRtpc",
        ],
        "SCItemFuelIntakeParams" => &["fuelPushRate", "minimumRate"],
        "ResourceContainer" => &[
            "capacity",
            "immutable",
            "mutabilityLevel",
            "generateRandomQuality",
            "inclusiveResources",
            "exclusiveResources",
            "inclusiveGroups",
            "exclusiveGroups",
            "defaultComposition",
            "defaultCompositionFillFactor",
            "hideFromUI",
        ],
        _ => &[],
    }
}

fn compare_tank(source: &StructValue<'_>, row: &Value) -> TestResult {
    for name in ["openState", "closedState", "pumpingState"] {
        let value = source.field(name);
        let cell = cell(row, name)?;
        if let Some(Original::Reference(link)) = value {
            require_state(cell, "present")?;
            let shape = proof::text(cell, "/value/type")?;
            if link.location().is_some() {
                assert_eq!(
                    shape, "weakReference",
                    "a local pointer is not a Record link"
                );
            }
            let guid_field = match shape {
                "weakReference" => "/value/value/recordGuid/value",
                "recordReference" => "/value/value/guid/value",
                _ => return Err("unrecognized tank state reference shape".into()),
            };
            assert!(
                proof::text(cell, guid_field)? == link.guid().to_string()
                    && proof::number(cell, "/value/value/targetRecordIndex/value")?
                        == u64::from(link.record_index()),
                "authored tank state reference retains physical identity"
            );
        } else {
            verify_state(value, cell)?;
        }
    }
    for (name, field) in [
        ("fuelFlowLoopStartAudioTrigger", "audioTrigger"),
        ("fuelFlowLoopStopAudioTrigger", "audioTrigger"),
        ("fuelFillDrainRateAudioRtpc", "rtpc"),
        ("fuelFillLevelAudioRtpc", "rtpc"),
    ] {
        owned(source, row, name, |source, row| {
            let cell = cell(row, field)?;
            match source.field(field) {
                Some(Original::String(value)) => {
                    require_state(cell, "present")?;
                    assert_eq!(
                        cell.get("value").and_then(Value::as_str),
                        Some(*value),
                        "original audio declaration"
                    );
                    Ok(())
                }
                other => verify_state(other, cell),
            }
        })?;
    }
    Ok(())
}

fn compare_policy(forge: &DataForge<'_>, source: &StructValue<'_>, row: &Value) -> TestResult {
    for name in ["immutable", "generateRandomQuality", "hideFromUI"] {
        let cell = cell(row, wire_name(name))?;
        match source.field(name) {
            Some(Original::Bool(value)) => {
                require_state(cell, "present")?;
                assert_eq!(
                    cell.get("value").and_then(Value::as_bool),
                    Some(*value),
                    "original {name}"
                );
            }
            other => verify_state(other, cell)?,
        }
    }
    let mutability = cell(row, "mutabilityLevel")?;
    match source.field("mutabilityLevel") {
        Some(Original::Enum(value)) => {
            require_state(mutability, "present")?;
            assert_eq!(
                mutability.get("value").and_then(Value::as_str),
                Some(*value),
                "authored enum option"
            );
        }
        other => verify_state(other, mutability)?,
    }
    for (name, family) in [
        ("inclusiveResources", "ResourceType"),
        ("exclusiveResources", "ResourceType"),
        ("inclusiveGroups", "ResourceTypeGroup"),
        ("exclusiveGroups", "ResourceTypeGroup"),
    ] {
        verify_array(source.field(name), cell(row, name)?, |source, row| {
            verify_link(forge, family, source, row)
        })?;
    }
    single(
        source,
        "defaultCompositionFillFactor",
        cell(row, "defaultCompositionFillFactor")?,
    )
}

fn wire_name(source_name: &str) -> &str {
    match source_name {
        "hideFromUI" => "hideFromUi",
        name => name,
    }
}
