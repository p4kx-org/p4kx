// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use anyhow::Result;
use p4kx_assembly::ArmourPart;
use p4kx_extraction::{
    DataForge, DataForgeOrigin, EntryOrigin, FieldOrigins, FieldPath, InputScope,
    SourceCoordinateOmission, SourceInput, SourceInventory, SourceLocation, SourceScope, SourceUse,
    SourceUseRole, SourceUses,
};
use serde::Serialize;

use super::{Done, GlbJson};
use crate::cli_armour::ArmourGlbArgs;
use crate::{json, scene_report_origins};

#[derive(Serialize)]
struct SourcedReport<'a> {
    #[serde(flatten)]
    report: GlbJson<'a>,
    #[serde(rename = "fieldOrigins")]
    fields: FieldOrigins,
    provenance: SourceInventory,
}

pub(super) fn print(
    done: &Done<'_>,
    args: &ArmourGlbArgs,
    forge: &DataForge<'_>,
    original: &DataForgeOrigin,
) -> Result<()> {
    let report = super::report(done, args);
    let uses = SourceUses::with_compact_use_refs();
    let mut fields = FieldOrigins::new();
    let tool = uses.origin(&[], true);
    for path in ["/archive", "/out", "/lod"] {
        fields.insert(FieldPath::parse(path)?, tool.clone())?;
    }
    if args.body.is_some() || done.selection.loadout.is_none() {
        for path in ["/body", "/skeleton"] {
            fields.insert(FieldPath::parse(path)?, tool.clone())?;
        }
    }

    let (scope, complete) = scene_report_origins::dataforge_scope(&uses, original)?;
    if let Some(loadout) = &done.selection.loadout {
        fields.insert(FieldPath::parse("/loadout/name")?, tool)?;
        if let Some(record) = loadout.record_index.and_then(|index| forge.record(index)) {
            let origin =
                scene_report_origins::record_name(&uses, &scope, forge, &record, complete)?;
            fields.insert(FieldPath::parse("/loadout/class_name")?, origin)?;
        }
    }
    if report.bones != 0
        && let Some(rig) = done
            .armour
            .skeleton_archive_source()
            .and_then(|model| model.origin())
    {
        let source = payload(&uses, rig)?;
        fields.insert(FieldPath::parse("/bones")?, uses.origin(&[source], false))?;
    }
    for (index, part) in done.armour.parts().iter().enumerate() {
        part_fields(&mut fields, &uses, &scope, index, part)?;
    }
    scene_report_origins::cover_leaves(
        &mut fields,
        &serde_json::to_value(&report)?,
        &mut FieldPath::root(),
        &uses.origin(&[], false),
    )?;
    json::print(&SourcedReport {
        report,
        fields,
        provenance: uses.freeze()?,
    })?;
    Ok(())
}

fn part_fields(
    fields: &mut FieldOrigins,
    uses: &SourceUses,
    scope: &SourceScope,
    index: usize,
    part: &ArmourPart,
) -> Result<()> {
    let file = selector(scope, part.geometry_selector())?;
    if let Some(selected) = file {
        fields.insert(
            FieldPath::parse(&format!("/parts/{index}/file"))?,
            uses.origin(&[selected], false),
        )?;
    }
    if part.materials().library().is_some() {
        let mut sources = Vec::new();
        sources.extend(selector(scope, part.material_selector())?);
        if let Some(library) = part.material_archive_origins().library() {
            sources.push(payload(uses, library)?);
        }
        fields.insert(
            FieldPath::parse(&format!("/parts/{index}/material"))?,
            uses.origin(&sources, false),
        )?;
    }
    Ok(())
}

fn selector(scope: &SourceScope, location: Option<&SourceLocation>) -> Result<Option<SourceUse>> {
    Ok(location
        .map(|location| scope.at(location.clone(), SourceUseRole::Selector))
        .transpose()?)
}

fn payload(uses: &SourceUses, entry: &EntryOrigin) -> Result<SourceUse> {
    Ok(uses
        .observe(SourceInput::Archive(entry), InputScope::Payload)?
        .at_opaque(
            SourceUseRole::Conversion,
            SourceCoordinateOmission::Unavailable,
        )?)
}
