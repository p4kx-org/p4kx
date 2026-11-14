// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::Result;
use p4kx_assembly::character::{CHF_SIZE, Chf, ChfError, NameSummary, NameTable, Versions};
use p4kx_extraction::{
    ArchiveWriteProtection, CancelToken, ObservedFileRead, read_observed_file_limit,
};
use serde::Serialize;
use thiserror::Error;

use crate::chf_names;
use crate::chf_origin_fields::{ChfField, ChfInputFormat, ChfOrigins, ProjectedChfOrigins};
use crate::chf_origin_io::{ChfPublication, ChfPublicationSummary};
use crate::cli::{ChfAction, ChfArgs};
use crate::cli_chf::ChfToJsonArgs;
use crate::cryxml::JsonNeedsOut;
use crate::exit;
use crate::json;

/// The largest JSON file `p4kx chf from-json` reads.
const MAX_CHF_JSON_LEN: u64 = 1024 * 1024;

/// The input file of `p4kx chf` cannot be read or is larger than any valid input.
#[derive(Debug, Error)]
pub(crate) enum UnreadableChfInput {
    /// Opening or reading failed.
    #[error("cannot read {}", path.display())]
    Io {
        /// The input file.
        path: PathBuf,
        /// What failed.
        #[source]
        source: io::Error,
    },
    /// The file is larger than any input of its kind.
    #[error("cannot read {}: it is larger than {limit} bytes", path.display())]
    TooLarge {
        /// The input file.
        path: PathBuf,
        /// The largest accepted input.
        limit: u64,
    },
}

/// The input of `p4kx chf to-json` is not a valid CHF file.
#[derive(Debug, Error)]
#[error("cannot read {} as CHF", path.display())]
pub(crate) struct InvalidChf {
    pub(crate) path: PathBuf,
    #[source]
    pub(crate) source: ChfError,
}

/// The input of `p4kx chf from-json` is not valid CHF JSON or cannot be written as CHF.
#[derive(Debug, Error)]
#[error("cannot convert {} to CHF", path.display())]
pub(crate) struct InvalidChfJson {
    path: PathBuf,
    #[source]
    source: ChfError,
}

#[derive(Serialize)]
struct Summary {
    input: String,
    output: String,
    versions: Versions,
    size: u64,
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    names: Option<NamesJson>,
}

#[derive(Serialize)]
struct NamesJson {
    named: usize,
    unresolved: usize,
    unresolved_hashes: Vec<String>,
}

#[derive(Serialize)]
struct JsonSummary {
    #[serde(flatten)]
    summary: Summary,
    #[serde(flatten)]
    publication: ChfPublicationSummary,
}

/// Runs `p4kx chf to-json` or `p4kx chf from-json`.
pub(crate) fn run(args: &ChfArgs, as_json: bool) -> Result<ExitCode> {
    let success = |()| ExitCode::from(exit::SUCCESS);
    match &args.action {
        ChfAction::ToJson(args) => to_json(args, as_json).map(success),
        ChfAction::FromJson { file, out } => from_json(file, out, as_json).map(success),
        ChfAction::Diff(args) => crate::chf_diff::run(args, as_json),
        ChfAction::Glb(args) => crate::chf_glb::run(args, as_json).map(success),
    }
}

fn to_json(args: &ChfToJsonArgs, as_json: bool) -> Result<()> {
    if as_json && args.out.is_none() {
        return Err(JsonNeedsOut.into());
    }
    let cancel = crate::extract::cancel_on_interrupt()?;
    let archive = args.archive.as_deref();
    let artifact_protection = archive
        .map(|path| ArchiveWriteProtection::new(path, cancel.flag()))
        .transpose()?;
    if args.out.is_none() && !args.origin_stderr && args.origin_out.is_none() {
        let bytes = read_capped(&args.file, CHF_SIZE as u64)?;
        let chf = read_chf(&args.file, &bytes)?;
        return legacy_stdout(&args.file, archive, &chf);
    }
    let input = read_capped_observed(&args.file, CHF_SIZE as u64)?;
    let chf = read_chf(&args.file, input.bytes())?;
    let NamedJson {
        text,
        summary,
        fields,
        origins,
    } = observed_json(args, &input, &chf)?;
    let publication = ChfPublication {
        bytes: text.as_bytes(),
        fields,
        inventory: origins.inventory()?,
    };
    if let Some(out) = args.out.as_deref() {
        let receipt = publication.file(
            &args.file,
            out,
            archive,
            artifact_protection.as_ref(),
            &cancel,
        )?;
        let provenance = as_json
            .then(|| publication.summary(receipt, &origins))
            .transpose()?;
        report(
            &args.file,
            out,
            chf.character.versions,
            text.len() as u64,
            Some(&summary),
            provenance,
        )
    } else {
        publication.stdout(
            &args.file,
            archive,
            args.origin_out.as_deref(),
            args.origin_stderr,
            &cancel,
        )?;
        if !args.origin_stderr {
            chf_names::note(&summary)?;
        }
        Ok(())
    }
}

struct NamedJson {
    text: String,
    summary: NameSummary,
    fields: ProjectedChfOrigins,
    origins: ChfOrigins,
}

fn observed_json(args: &ChfToJsonArgs, input: &ObservedFileRead, chf: &Chf) -> Result<NamedJson> {
    let origins = ChfOrigins::new(input, ChfInputFormat::Chf)?;
    let names = args
        .archive
        .as_deref()
        .map(|path| chf_names::observed(path, chf, origins.uses()))
        .transpose()?;
    let builtin = NameTable::builtin();
    let table = names
        .as_ref()
        .map_or(&builtin, chf_names::ObservedNames::table);
    let mut fields = Vec::new();
    let (mut text, summary) = chf
        .to_json_named_observed(table, |field| fields.push(ChfField::observed(field)))
        .map_err(|source| InvalidChf {
            path: args.file.clone(),
            source,
        })?;
    text.push('\n');
    let fields = origins.json_fields(fields, names.as_ref())?;
    Ok(NamedJson {
        text,
        summary,
        fields,
        origins,
    })
}

pub(crate) fn read_chf(file: &Path, bytes: &[u8]) -> Result<Chf, InvalidChf> {
    Chf::read(bytes).map_err(|source| InvalidChf {
        path: file.to_owned(),
        source,
    })
}

fn legacy_stdout(file: &Path, archive: Option<&Path>, chf: &Chf) -> Result<()> {
    let names = chf_names::names_for(archive, chf)?;
    let (mut text, summary) = chf.to_json_named(&names).map_err(|source| InvalidChf {
        path: file.to_owned(),
        source,
    })?;
    text.push('\n');
    io::stdout().lock().write_all(text.as_bytes())?;
    Ok(chf_names::note(&summary)?)
}

fn from_json(file: &Path, out: &Path, as_json: bool) -> Result<()> {
    let input = read_capped_observed(file, MAX_CHF_JSON_LEN)?;
    let invalid = |source| InvalidChfJson {
        path: file.to_owned(),
        source,
    };
    let chf = Chf::from_json(input.bytes()).map_err(invalid)?;
    let bytes = chf.write().map_err(invalid)?;
    let origins = ChfOrigins::new(&input, ChfInputFormat::Json)?;
    let publication = ChfPublication {
        bytes: &bytes,
        fields: origins.opaque_output()?,
        inventory: origins.inventory()?,
    };
    let receipt = publication.file(file, out, None, None, &CancelToken::new())?;
    let provenance = as_json
        .then(|| publication.summary(receipt, &origins))
        .transpose()?;
    report(
        file,
        out,
        chf.character.versions,
        bytes.len() as u64,
        None,
        provenance,
    )
}

fn report(
    input: &Path,
    output: &Path,
    versions: Versions,
    size: u64,
    names: Option<&NameSummary>,
    publication: Option<ChfPublicationSummary>,
) -> Result<()> {
    let summary = Summary {
        input: json::path(input),
        output: json::path(output),
        versions,
        size,
        names: names.map(|summary| NamesJson {
            named: summary.named,
            unresolved: summary.unresolved,
            unresolved_hashes: chf_names::hex_list(&summary.unresolved_hashes),
        }),
    };
    if let Some(publication) = publication {
        json::print(&JsonSummary {
            summary,
            publication,
        })?;
    } else {
        writeln!(
            io::stdout().lock(),
            "converted {} (versions {}/{}) to {}, {} bytes",
            summary.input,
            versions.female,
            versions.male,
            summary.output,
            size
        )?;
        if let Some(names) = names {
            writeln!(io::stdout().lock(), "{}", chf_names::sentence(names))?;
        }
    }
    Ok(())
}

/// Reads a whole file of at most `limit` bytes.
pub(crate) fn read_capped(path: &Path, limit: u64) -> Result<Vec<u8>, UnreadableChfInput> {
    let io_error = |source| UnreadableChfInput::Io {
        path: path.to_owned(),
        source,
    };
    let mut bytes = Vec::new();
    File::open(path)
        .and_then(|file| file.take(limit + 1).read_to_end(&mut bytes))
        .map_err(io_error)?;
    if bytes.len() as u64 > limit {
        return Err(UnreadableChfInput::TooLarge {
            path: path.to_owned(),
            limit,
        });
    }
    Ok(bytes)
}

pub(crate) fn read_capped_observed(
    path: &Path,
    limit: u64,
) -> Result<ObservedFileRead, UnreadableChfInput> {
    let input =
        read_observed_file_limit(path, limit + 1).map_err(|source| UnreadableChfInput::Io {
            path: path.to_owned(),
            source,
        })?;
    if input.bytes().len() as u64 > limit {
        return Err(UnreadableChfInput::TooLarge {
            path: path.to_owned(),
            limit,
        });
    }
    Ok(input)
}
