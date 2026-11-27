// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{ArchiveWatch, DataForge, StructValue, Value as Original};

use super::{ATTACHABLE, CONSUMABLE, LASER, TestResult};
use crate::authored_component_witness::{ComponentCandidate, select_component_owners};

pub(super) fn discover<'source>(
    forge: &'source DataForge<'source>,
    watch: &ArchiveWatch,
) -> TestResult<[u32; 3]> {
    let mut candidates = [
        ComponentCandidate::new(forge, LASER, laser)?.requiring_every_occurrence(),
        ComponentCandidate::new(forge, ATTACHABLE, attachable)?.requiring_every_occurrence(),
        ComponentCandidate::new(forge, CONSUMABLE, consumable)?.requiring_every_occurrence(),
    ];
    select_component_owners(forge, watch, &mut candidates, |_| Ok(true))?;
    let [laser, attachable, consumable] = candidates;
    Ok([
        laser.owner("a readable mining laser with authored throttle values")?,
        attachable.owner("a readable attachable modifier with a typed original effect")?,
        consumable.owner("a readable consumable with authored contents and capacity")?,
    ])
}

fn laser(value: &StructValue<'_>) -> bool {
    ["throttleMinimum", "throttleLerpSpeed"]
        .iter()
        .all(|field| matches!(value.field(field), Some(Original::Single(_))))
        && known_reference(value.field("globalParams"), "MiningLaserGlobalParams")
}

fn attachable(value: &StructValue<'_>) -> bool {
    let Some(Original::Array(slots)) = value.field("modifiers") else {
        return false;
    };
    if !matches!(value.field("activationMethod"), Some(Original::Enum(_))) {
        return false;
    }
    let mut typed = false;
    for slot in slots {
        let Original::Struct(effect) = slot else {
            if matches!(slot, Original::Null) {
                continue;
            }
            return false;
        };
        match effect.type_name() {
            "ItemMiningModifierParams"
            | "ItemMineableRockModifierParams"
            | "MiningFilterItemModifierParams" => typed = true,
            "ItemWeaponModifiersParams" => return false,
            _ => {}
        }
    }
    typed
}

fn consumable(value: &StructValue<'_>) -> bool {
    let Some(Original::Struct(volume)) = value.field("consumableVolume") else {
        return false;
    };
    let volume = match volume.type_name() {
        "SMicroCargoUnit" => matches!(volume.field("microSCU"), Some(Original::Int32(_))),
        "SCentiCargoUnit" => matches!(volume.field("centiSCU"), Some(Original::Int32(_))),
        "SStandardCargoUnit" => matches!(
            volume.field("standardCargoUnits"),
            Some(Original::Single(_))
        ),
        _ => false,
    };
    let Some(Original::Array(contents)) = value.field("defaultContents") else {
        return false;
    };
    volume
        && contents.iter().any(|raw| match raw {
            Original::Struct(slot) => {
                known_reference(slot.field("consumableSubtype"), "ConsumableSubtype")
            }
            _ => false,
        })
        && contents.iter().all(|raw| {
            let Original::Struct(slot) = raw else {
                return false;
            };
            matches!(slot.field("ratio"), Some(Original::Single(_)))
                && usable_link(slot.field("consumableSubtype"), "ConsumableSubtype")
        })
}

fn usable_link(value: Option<&Original<'_>>, family: &str) -> bool {
    match value {
        Some(Original::Reference(target)) => {
            target.type_name() == family && target.location().is_none()
        }
        None | Some(Original::Null) => true,
        _ => false,
    }
}

fn known_reference(value: Option<&Original<'_>>, family: &str) -> bool {
    matches!(
        value, Some(Original::Reference(target))
            if target.type_name() == family && target.location().is_none()
    )
}
