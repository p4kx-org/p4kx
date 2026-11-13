// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, BufWriter, Write};
use std::path::Path;

use anyhow::{Context, Result};
use p4kx_export::{
    ActionMapCounts, ActionMapFormat, ActionMapLabels, ActionMapProfile,
    action_map_summary_origins, write_action_maps, write_action_maps_with_origins,
};
use p4kx_extraction::{
    Archive, ArchiveWriteProtection, CancelToken, FieldOrigins, LocalizationSet,
    OriginArtifactBinding, OriginDigestWriter, SourceInventory,
};
use serde::Serialize;

use crate::cli_actionmap::{ActionMapAction, ActionMapArgs, ActionMapExportArgs};
use crate::file_receipt::{FileReceipt, StdoutReceipt};
use crate::json;
use crate::loc::{LanguageNotInArchive, print_notes};

#[derive(Serialize)]
struct ExportJson<'a> {
    archive: String,
    out: String,
    format: &'static str,
    language: &'a str,
    action_maps: usize,
    actions: usize,
    unresolved_labels: usize,
    origin_receipt: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonReport<'a> {
    #[serde(flatten)]
    data: ExportJson<'a>,
    field_origins: FieldOrigins,
    provenance: SourceInventory,
}

/// Runs `p4kx actionmap`.
pub(crate) fn run(args: &ActionMapArgs, as_json: bool) -> Result<()> {
    match &args.action {
        ActionMapAction::Export(args) => export(args, as_json),
    }
}

fn export(args: &ActionMapExportArgs, as_json: bool) -> Result<()> {
    let format = args.library_format()?;
    let archive = crate::key::open(&args.archive)?;
    crate::guard::ensure_writable_if(args.out.as_deref())?;
    crate::guard::ensure_writable_if(args.origin_out.as_deref())?;
    let profile = ActionMapProfile::read(&archive)?;
    let set = LocalizationSet::read(&archive, &[args.language])?;
    let entry = set.entry(args.language).ok_or(LanguageNotInArchive {
        language: args.language,
    })?;
    let labels = ActionMapLabels::new(entry).map_err(|source| {
        p4kx_extraction::LocalizationLoadError::Index {
            path: entry.path().to_owned(),
            source,
        }
    })?;
    print_notes(&set.notes(labels.texts()));
    let Some(path) = &args.out else {
        return stdout(args, archive.path(), format, &profile, &labels);
    };
    let summary_origins = as_json
        .then(|| action_map_summary_origins(&profile, &labels))
        .transpose()?;
    let cancel = CancelToken::new();
    let protection = ArchiveWriteProtection::new(archive.path(), cancel.flag())?;
    let receipt = FileReceipt::new_with_cancel(path, Some(&protection), Some(&protection), cancel)?;
    let (counts, _) = receipt
        .publish(
            |out| write_action_maps_with_origins(&profile, &labels, format, out),
            |out, binding, (_, origins)| origins.write(out, binding),
        )
        .with_context(|| format!("cannot write {}", path.display()))?;
    let name = receipt
        .path()
        .file_name()
        .context("missing origin companion basename")?;
    report(
        args,
        path,
        summary_origins,
        &archive,
        &counts,
        &json::path(Path::new(name)),
    )
}

fn stdout(
    args: &ActionMapExportArgs,
    archive: &Path,
    format: ActionMapFormat,
    profile: &ActionMapProfile,
    labels: &ActionMapLabels<'_>,
) -> Result<()> {
    let mut out = BufWriter::new(io::stdout().lock());
    if args.origin_out.is_none() && !args.origin_stderr {
        write_action_maps(
            profile.maps(),
            labels.texts(),
            args.language,
            format,
            &mut out,
        )?;
        return Ok(out.flush()?);
    }
    let cancel = CancelToken::new();
    let protection = args
        .origin_out
        .as_ref()
        .map(|_| ArchiveWriteProtection::new(archive, cancel.flag()))
        .transpose()?;
    let durable = args
        .origin_out
        .as_ref()
        .map(|path| StdoutReceipt::new(path, protection.as_ref()))
        .transpose()?;
    let mut measured = OriginDigestWriter::new(out);
    let (_, origins) = write_action_maps_with_origins(profile, labels, format, &mut measured)?;
    let binding = OriginArtifactBinding::Stdout {
        digest: measured.finish()?,
    };
    if let Some(durable) = durable {
        durable.publish(|out| origins.write(out, &binding))?;
    }
    if args.origin_stderr {
        origins.write(&mut BufWriter::new(io::stderr().lock()), &binding)?;
    }
    Ok(())
}

fn report(
    args: &ActionMapExportArgs,
    path: &Path,
    summary_origins: Option<(FieldOrigins, SourceInventory)>,
    archive: &Archive,
    counts: &ActionMapCounts,
    origin_receipt: &str,
) -> Result<()> {
    if let Some((field_origins, provenance)) = summary_origins {
        json::print(&JsonReport {
            data: ExportJson {
                archive: json::path(archive.path()),
                out: json::path(path),
                format: args.format.name(),
                language: args.language.code(),
                action_maps: counts.action_maps,
                actions: counts.actions,
                unresolved_labels: counts.unresolved_labels,
                origin_receipt: origin_receipt.to_owned(),
            },
            field_origins,
            provenance,
        })?;
    } else {
        writeln!(
            io::stdout().lock(),
            "wrote {}: {} actions in {} action maps, unresolved labels in {}: {}; origin: {}",
            path.display(),
            counts.actions,
            counts.action_maps,
            args.language,
            counts.unresolved_labels,
            origin_receipt,
        )?;
    }
    Ok(())
}
