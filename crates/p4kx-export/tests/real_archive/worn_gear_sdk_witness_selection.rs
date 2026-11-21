// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{ArchiveWatch, DataForge, PointerProbe, TreeBuilder, Value as Original};

use super::super::{
    CLOTHING, RADAR, SUIT, TestResult, all_gear_values_present, body, macro_values_present,
};

pub(super) fn discover<'source>(
    forge: &'source DataForge<'source>,
    watch: &ArchiveWatch,
) -> TestResult<[u32; 3]> {
    let mut candidates = [
        Candidate::new(forge, SUIT)?,
        Candidate::new(forge, CLOTHING)?,
        Candidate::new(forge, RADAR)?,
    ];
    let mut trees = TreeBuilder::new();
    for record in forge.records() {
        watch
            .poll()
            .map_err(|_| "Archive changed during worn gear discovery")?;
        if record.type_name().ok() != Some("EntityClassDefinition") {
            continue;
        }
        if !candidates.iter().any(|candidate| {
            candidate.owner.is_none() && trees.holds(&candidate.probe, &record).ok() == Some(true)
        }) {
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
        for candidate in &mut candidates {
            if candidate.owner.is_none()
                && trees.holds(&candidate.probe, &record).ok() == Some(true)
                && candidate.matches(forge, components)
            {
                candidate.owner = Some(record.index());
            }
        }
        if candidates.iter().all(|candidate| candidate.owner.is_some()) {
            break;
        }
    }
    let [suit, clothing, radar] = candidates;
    Ok([
        suit.owner
            .ok_or("a readable suit armor component with a typed resistance macro")?,
        clothing
            .owner
            .ok_or("a readable clothing component with typed resistance fields")?,
        radar
            .owner
            .ok_or("a readable item radar component with typed assist and ping fields")?,
    ])
}

struct Candidate<'source> {
    kind: &'static str,
    probe: PointerProbe<'source>,
    owner: Option<u32>,
}

impl<'source> Candidate<'source> {
    fn new(forge: &'source DataForge<'source>, kind: &'static str) -> TestResult<Self> {
        let probe = forge
            .strict_pointer_probe("Components", kind)
            .ok_or("the installed DataForge must declare the worn gear component")?;
        Ok(Self {
            kind,
            probe,
            owner: None,
        })
    }

    fn matches(
        &self,
        forge: &'source DataForge<'source>,
        components: &[Original<'source>],
    ) -> bool {
        let mut matched = false;
        for component in components.iter().filter_map(|raw| body(raw).ok()) {
            if component.type_name() != self.kind {
                continue;
            }
            if !all_gear_values_present(component, self.kind) {
                return false;
            }
            if self.kind == SUIT {
                let Some(Original::Reference(reference)) = component.field("damageResistance")
                else {
                    return false;
                };
                if reference.type_name() != "DamageResistanceMacro"
                    || forge
                        .record(reference.record_index())
                        .and_then(|record| record.tree().ok())
                        .is_none_or(|tree| !macro_values_present(&tree))
                {
                    return false;
                }
            }
            matched = true;
        }
        matched
    }
}
