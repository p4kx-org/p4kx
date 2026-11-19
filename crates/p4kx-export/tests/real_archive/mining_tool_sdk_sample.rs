// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    ArchiveWatch, DataForge, PointerProbe, StructValue, TreeBuilder, Value as Original,
};

use super::{ATTACHABLE, CONSUMABLE, LASER, TestResult};

pub(super) fn discover<'source>(
    forge: &'source DataForge<'source>,
    watch: &ArchiveWatch,
) -> TestResult<[u32; 3]> {
    let mut candidates = [
        Candidate::new(forge, LASER, laser)?,
        Candidate::new(forge, ATTACHABLE, attachable)?,
        Candidate::new(forge, CONSUMABLE, consumable)?,
    ];
    let mut trees = TreeBuilder::new();
    for record in forge.records() {
        watch
            .poll()
            .map_err(|_| "Archive changed during equipment discovery")?;
        if record.type_name().ok() != Some("EntityClassDefinition") {
            continue;
        }
        let matches = candidates.each_ref().map(|candidate| {
            candidate.owner.is_none() && trees.holds(&candidate.probe, &record).ok() == Some(true)
        });
        if !matches.contains(&true) {
            continue;
        }
        let Ok(tree) = trees.build(&record) else {
            continue;
        };
        let Some(Original::Array(components)) = tree.field("Components") else {
            continue;
        };
        if !components
            .iter()
            .all(|value| matches!(value, Original::Struct(_) | Original::Null))
        {
            continue;
        }
        for (candidate, held) in candidates.iter_mut().zip(matches) {
            if held && candidate.matches(components) {
                candidate.owner = Some(record.index());
            }
        }
        if candidates.iter().all(|candidate| candidate.owner.is_some()) {
            break;
        }
    }
    let [laser, attachable, consumable] = candidates;
    Ok([
        laser
            .owner
            .ok_or("a readable mining laser with authored throttle values")?,
        attachable
            .owner
            .ok_or("a readable attachable modifier with a typed original effect")?,
        consumable
            .owner
            .ok_or("a readable consumable with authored contents and capacity")?,
    ])
}

struct Candidate<'source> {
    kind: &'static str,
    probe: PointerProbe<'source>,
    qualifies: fn(&StructValue<'_>) -> bool,
    owner: Option<u32>,
}

impl<'source> Candidate<'source> {
    fn new(
        forge: &'source DataForge<'source>,
        kind: &'static str,
        qualifies: fn(&StructValue<'_>) -> bool,
    ) -> TestResult<Self> {
        let probe = forge
            .strict_pointer_probe("Components", kind)
            .ok_or("the installed DataForge must declare the required component family")?;
        Ok(Self {
            kind,
            probe,
            qualifies,
            owner: None,
        })
    }

    fn matches(&self, components: &[Original<'source>]) -> bool {
        let mut matched = false;
        for slot in components {
            if let Original::Struct(value) = slot
                && value.type_name() == self.kind
            {
                if !(self.qualifies)(value) {
                    return false;
                }
                matched = true;
            }
        }
        matched
    }
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
