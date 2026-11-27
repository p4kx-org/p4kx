// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use anyhow::Result;
use p4kx_assembly::EntityNode;
use p4kx_extraction::{
    DataForge, DataForgeOrigin, FieldOrigins, FieldPath, Record, SourceInventory, SourceUses,
};
use serde::Serialize;

use super::PackageJson;
use crate::{json, scene_report_origins};

#[derive(Serialize)]
struct SourcedReport<'a> {
    #[serde(flatten)]
    report: PackageJson<'a>,
    #[serde(rename = "fieldOrigins")]
    fields: FieldOrigins,
    provenance: SourceInventory,
}

pub(super) fn print(
    report: PackageJson<'_>,
    forge: &DataForge<'_>,
    record: &Record<'_>,
    original: &DataForgeOrigin,
    tree: &EntityNode,
) -> Result<()> {
    let uses = SourceUses::with_compact_use_refs();
    let mut fields = FieldOrigins::new();
    scene_report_origins::entity_identity(&mut fields, &uses, forge, record, original, tree)?;
    let tool = uses.origin(&[], true);
    for path in ["/archive", "/out", "/lod", "/texture_mip"] {
        fields.insert(FieldPath::parse(path)?, tool.clone())?;
    }
    if !report.excluded.is_empty() {
        fields.insert(FieldPath::parse("/excluded")?, tool.clone())?;
    }
    if !report.replaced.is_empty() {
        fields.insert(FieldPath::parse("/replaced")?, tool)?;
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
