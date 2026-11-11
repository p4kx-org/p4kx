// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use anyhow::Result;
use p4kx_assembly::Armour;
use p4kx_assembly::character::Dna;
use p4kx_extraction::{
    InputScope, ObservedFileRead, RiggedGlbEmission, SourceInput, SourceLocation, SourceScope,
    SourceUseRole, SourceUses,
};

use super::Contributors;

pub(super) fn whole(uses: &SourceUses, input: &ObservedFileRead) -> Result<Contributors> {
    let scope = observe(uses, input)?;
    Ok(Contributors {
        values: vec![scope.at(location("character".into()), SourceUseRole::Conversion)?],
        complete: input.origin().is_some(),
    })
}

pub(super) fn shape(
    uses: &SourceUses,
    input: &ObservedFileRead,
    dna: Option<&Dna>,
    armour: &Armour,
    emission: &RiggedGlbEmission,
    bones_written: bool,
) -> Result<Contributors> {
    let mut found = Contributors::default();
    if !armour
        .parts()
        .iter()
        .zip(emission.parts())
        .any(|(part, emitted)| {
            !part.shape_archive_sources().is_empty()
                && (emitted.geometry() || (bones_written && part.has_facial_rig()))
        })
    {
        return Ok(found);
    }
    let Some(dna) = dna else {
        found.incomplete();
        return Ok(found);
    };
    let scope = observe(uses, input)?;
    found.complete = input.origin().is_some();
    found.values.push(scope.at(
        location("/character/dna/gender_hash".into()),
        SourceUseRole::Selector,
    )?);
    for (part, blends) in dna.face_parts.iter().enumerate() {
        for (index, _) in blends
            .iter()
            .enumerate()
            .filter(|(_, blend)| blend.value != 0)
        {
            for (field, role) in [
                ("value", SourceUseRole::Conversion),
                ("head_id", SourceUseRole::Selector),
            ] {
                found.values.push(scope.at(
                    location(format!(
                        "/character/dna/face_parts/{part}/blends/{index}/{field}"
                    )),
                    role,
                )?);
            }
        }
    }
    Ok(found)
}

fn observe(uses: &SourceUses, input: &ObservedFileRead) -> Result<SourceScope> {
    Ok(uses.observe(
        input
            .origin()
            .map_or(SourceInput::Unknown, SourceInput::File),
        InputScope::Payload,
    )?)
}

fn location(field: String) -> SourceLocation {
    SourceLocation::Subresource {
        namespace: "chf".into(),
        indices: Vec::new(),
        field,
    }
}
