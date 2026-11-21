// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::process::ExitCode;

use anyhow::Result;
use p4kx_extraction::{
    FieldOrigins, FieldPath, InputScope, SourceInventory, SourceLocation, SourceUseRole, SourceUses,
};
use serde::Serialize;

use crate::cli::{DcbArgs, DcbCommand, DcbInfoArgs};
use crate::forge_input::ForgeInput;
use crate::{dcb_export, dcb_fields, dcb_query, dcb_refs, dcb_tags, exit, json};

#[derive(Serialize)]
struct Summary<'a> {
    archive: String,
    entry: &'a str,
    version: u32,
    record_count: usize,
    struct_count: usize,
    enum_count: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonSummary<'a> {
    #[serde(flatten)]
    summary: Summary<'a>,
    field_origins: FieldOrigins,
    provenance: SourceInventory,
}

/// Runs a `p4kx dcb` subcommand.
pub(crate) fn run(args: &DcbArgs, as_json: bool) -> Result<ExitCode> {
    match &args.command {
        DcbCommand::Info(args) => info(args, as_json).map(|()| ExitCode::from(exit::SUCCESS)),
        DcbCommand::Export(args) => dcb_export::run(args, as_json),
        DcbCommand::Query(args) => dcb_query::run(args, as_json),
        DcbCommand::Refs(args) => dcb_refs::run(args, as_json),
        DcbCommand::Tags(args) => dcb_tags::run(args, as_json),
        DcbCommand::Fields(args) => dcb_fields::run(args, as_json),
    }
}

/// Prints the DataForge version and its number of Records, structs and enums.
fn info(args: &DcbInfoArgs, as_json: bool) -> Result<()> {
    let input = ForgeInput::open(&args.archive)?;
    let forge = input.entry().open()?;
    let summary = Summary {
        archive: json::path(input.path()),
        entry: input.entry().path(),
        version: forge.version(),
        record_count: forge.record_count(),
        struct_count: forge.struct_count(),
        enum_count: forge.enum_count(),
    };
    if as_json {
        json::print(&json_summary(&input, summary)?)?;
    } else {
        print_lines(&summary, input.archive().is_some())?;
    }
    Ok(())
}

fn json_summary<'summary>(
    input: &ForgeInput,
    summary: Summary<'summary>,
) -> Result<JsonSummary<'summary>> {
    let uses = SourceUses::new();
    let scope = uses.observe(input.source_input()?, InputScope::Payload)?;
    let mut fields = FieldOrigins::new();
    for (path, field) in [
        ("/version", "version"),
        ("/record_count", "recordCount"),
        ("/struct_count", "structCount"),
        ("/enum_count", "enumCount"),
    ] {
        let datum = scope.at(
            SourceLocation::Subresource {
                namespace: "dataforgeHeader".to_owned(),
                indices: Vec::new(),
                field: field.to_owned(),
            },
            SourceUseRole::Payload,
        )?;
        fields.insert(FieldPath::parse(path)?, uses.origin(&[datum], true))?;
    }
    let name_scope = if input.archive().is_some() {
        InputScope::ArchiveDirectoryMetadata
    } else {
        InputScope::FileMetadata
    };
    let name = scope.for_scope(name_scope).at(
        SourceLocation::Metadata {
            field: "path".to_owned(),
        },
        SourceUseRole::Payload,
    )?;
    fields.insert(FieldPath::parse("/entry")?, uses.origin(&[name], true))?;
    fields.insert(FieldPath::parse("/archive")?, uses.origin(&[], true))?;
    Ok(JsonSummary {
        summary,
        field_origins: fields,
        provenance: uses.freeze()?,
    })
}

fn print_lines(summary: &Summary<'_>, from_archive: bool) -> io::Result<()> {
    let source = if from_archive { "Archive" } else { "File" };
    let lines = [
        (source, summary.archive.clone()),
        ("Entry", summary.entry.to_owned()),
        ("Version", summary.version.to_string()),
        ("Records", summary.record_count.to_string()),
        ("Structs", summary.struct_count.to_string()),
        ("Enums", summary.enum_count.to_string()),
    ];
    let mut out = io::stdout().lock();
    for (label, value) in lines {
        writeln!(out, "{:<16} {value}", format!("{label}:"))?;
    }
    Ok(())
}
