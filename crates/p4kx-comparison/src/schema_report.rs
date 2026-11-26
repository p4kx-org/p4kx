// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use p4kx_channels::PublicPatches;
use p4kx_extraction::SourceUses;
use schemars::JsonSchema;
use serde::Serialize;

use crate::report::{Sources, sources};
use crate::report_origins::{ReportOriginFields, ReportOrigins};
use crate::schema_diff;
use crate::schema_report_origins::{self, SchemaSource};
use crate::schema_side::SideSchema;
use crate::schema_witness::SchemaWitness;
use crate::{Archive, DataForgeEntry, DataForges, SchemaDiff, Side};

/// The version of the schema drift report shape; it grows only when the shape changes
/// incompatibly.
pub const SCHEMA_REPORT_FORMAT_VERSION: u32 = 1;

/// The JSON report of the schema drift between the DataForges of an old and a new Archive.
#[derive(Debug, Serialize, JsonSchema)]
#[schemars(title = "P4KX schema drift report")]
pub struct SchemaReport<'a> {
    /// The version of the report shape.
    format_version: u32,
    /// The two Archives compared.
    source: Sources<'a>,
    /// The schema changes.
    #[serde(flatten)]
    #[schemars(with = "SchemaDiff")]
    diff: Cow<'a, SchemaDiff>,
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<ReportOriginFields>")]
    origins: Option<ReportOrigins>,
}

impl SchemaReport<'_> {
    /// Returns whether the report contains differences.
    #[must_use]
    pub fn has_differences(&self) -> bool {
        self.diff.has_differences()
    }
}

/// Returns the JSON report of `diff`, the schema drift from the `old` to the `new` Archive, with
/// the public patches `patches` knows for their Builds.
#[must_use]
pub fn schema_report<'a>(
    old: &'a Archive,
    new: &'a Archive,
    diff: &'a SchemaDiff,
    patches: Option<&PublicPatches>,
) -> SchemaReport<'a> {
    SchemaReport {
        format_version: SCHEMA_REPORT_FORMAT_VERSION,
        source: schema_sources(old, new, patches),
        diff: Cow::Borrowed(diff),
        origins: None,
    }
}

/// Returns the schema drift and field origins of the compared DataForge Entries.
/// An Entry must match its parsed DataForge and Archive to supply original coordinates.
#[must_use]
pub fn schema_report_with_dataforge_entries<'a>(
    old: &'a Archive,
    new: &'a Archive,
    dataforges: DataForges<'_>,
    entries: [&DataForgeEntry; 2],
    patches: Option<&PublicPatches>,
) -> SchemaReport<'a> {
    let forges = [dataforges.old, dataforges.new];
    let (before, after) = rayon::join(
        || SideSchema::read(forges[0]),
        || SideSchema::read(forges[1]),
    );
    let diff = schema_diff::compare_sides(&before, &after);
    let origins = (|| {
        let uses = SourceUses::with_compact_use_refs();
        let witness = |archive, forge, entry: &DataForgeEntry| {
            let origin = entry
                .origin_of(forge)
                .filter(|origin| origin.matches_archive(archive));
            SchemaWitness::new(&uses, forge, origin)
        };
        let left = witness(old, forges[0], entries[0])?;
        let right = witness(new, forges[1], entries[1])?;
        schema_report_origins::capture(
            &diff,
            &uses,
            [
                SchemaSource {
                    schema: &before,
                    witness: &left,
                },
                SchemaSource {
                    schema: &after,
                    witness: &right,
                },
            ],
        )
    })();
    SchemaReport {
        format_version: SCHEMA_REPORT_FORMAT_VERSION,
        source: schema_sources(old, new, patches),
        diff: Cow::Owned(diff),
        origins: Some(ReportOrigins(origins)),
    }
}

fn schema_sources<'a>(
    old: &'a Archive,
    new: &'a Archive,
    patches: Option<&PublicPatches>,
) -> Sources<'a> {
    sources(
        &Side::of(old).with_public_patches(patches),
        &Side::of(new).with_public_patches(patches),
    )
}

/// Returns the JSON Schema (draft 2020-12) of [`SchemaReport`].
#[must_use]
pub fn schema_report_schema() -> serde_json::Value {
    schemars::schema_for!(SchemaReport<'static>).to_value()
}
