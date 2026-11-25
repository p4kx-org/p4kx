// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use anyhow::Result;
use p4kx_extraction::{Archive, FieldOrigins, FieldPath, SourceInventory, SourceUses};
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

pub(super) fn print(report: PackageJson<'_>, layers: &[Archive]) -> Result<()> {
    let uses = SourceUses::with_compact_use_refs();
    let mut fields = FieldOrigins::new();
    scene_report_origins::container_paths(&uses, &mut fields, layers, report.containers, |_| {
        Ok(())
    })?;
    let tool = uses.origin(&[], true);
    for path in ["/archive", "/out", "/lod", "/texture_mip"] {
        fields.insert(FieldPath::parse(path)?, tool.clone())?;
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
