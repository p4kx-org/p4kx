// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{ArchiveWatch, DataForge, PointerProbe, TreeBuilder, Value as Original};

use super::{CONTROLLER_TYPE, IFCS_TYPE, Result, THRUSTER_TYPE};

#[derive(Clone, Copy)]
pub(super) struct WitnessCase<const N: usize> {
    pub(super) kind: &'static str,
    pub(super) fields: &'static [&'static str; N],
}

pub(super) const CONTROLLERS: WitnessCase<1> = WitnessCase {
    kind: CONTROLLER_TYPE,
    fields: &["fuelWarningDisplayTime"],
};
pub(super) const IFCS: WitnessCase<1> = WitnessCase {
    kind: IFCS_TYPE,
    fields: &["maxSpeed"],
};
pub(super) const THRUSTERS: WitnessCase<2> = WitnessCase {
    kind: THRUSTER_TYPE,
    fields: &["thrustCapacity", "thrustCapacityNew"],
};

pub(super) fn discover<'source>(
    forge: &'source DataForge<'source>,
    watch: &ArchiveWatch,
) -> Result<[u32; 3]> {
    let mut candidates = [
        Candidate::new(forge, CONTROLLERS)?,
        Candidate::new(forge, IFCS)?,
        Candidate::new(forge, THRUSTERS)?,
    ];
    let mut trees = TreeBuilder::new();
    for record in forge.records() {
        watch.poll()?;
        if record.type_name().ok() != Some("EntityClassDefinition") {
            continue;
        }
        let possible = candidates.iter().any(|candidate| {
            candidate.owner.is_none() && trees.holds(&candidate.probe, &record).ok() == Some(true)
        });
        if !possible {
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
            if candidate.owner.is_none() && candidate.positive(components) {
                candidate.owner = Some(record.index());
            }
        }
        if candidates.iter().all(|candidate| candidate.owner.is_some()) {
            break;
        }
    }
    let [controller, ifcs, thruster] = candidates;
    Ok([
        controller
            .owner
            .ok_or("no readable Flight Controller warning-time witness")?,
        ifcs.owner.ok_or("no readable IFCS maximum-speed witness")?,
        thruster
            .owner
            .ok_or("no readable Thruster capacity witness")?,
    ])
}

struct Candidate<'source> {
    name: &'static str,
    fields: &'static [&'static str],
    probe: PointerProbe<'source>,
    owner: Option<u32>,
}

impl<'source> Candidate<'source> {
    fn new<const N: usize>(
        forge: &'source DataForge<'source>,
        case: WitnessCase<N>,
    ) -> Result<Self> {
        Ok(Self {
            name: case.kind,
            fields: case.fields,
            probe: forge
                .pointer_probe("Components", case.kind)
                .ok_or("no instantiated declared Flight component for this witness")?,
            owner: None,
        })
    }

    fn positive(&self, components: &[Original<'source>]) -> bool {
        let mut positive = false;
        for value in components {
            let Original::Struct(component) = value else {
                continue;
            };
            if component.type_name() != self.name {
                continue;
            }
            for field in self.fields {
                match component.field(field) {
                    Some(Original::Single(_)) => positive = true,
                    Some(Original::Null) | None => {}
                    _ => return false,
                }
            }
        }
        positive
    }
}
