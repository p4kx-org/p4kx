// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use p4kx_extraction::cryxml::{self, Format, Xml};
use serde::Serialize;
use thiserror::Error;

use crate::cli_cryxml::CryXmlArgs;
use crate::json;

/// The input file of `p4kx cryxml` cannot be read.
#[derive(Debug, Error)]
#[error("cannot read {}", path.display())]
pub(crate) struct UnreadableInput {
    path: PathBuf,
    #[source]
    source: io::Error,
}

/// `--json` was given without `--out`, so the XML and the JSON would share stdout.
#[derive(Debug, Error)]
#[error("--json needs --out: the XML and the JSON cannot both go to stdout")]
pub(crate) struct JsonNeedsOut;

#[derive(Serialize)]
struct Summary {
    input: String,
    output: String,
    format: &'static str,
    byte_order: Option<&'static str>,
}

/// Converts a `CryXml` file to XML on stdout or in `--out`, passing text XML through.
pub(crate) fn run(args: &CryXmlArgs, as_json: bool) -> Result<()> {
    if as_json && args.out.is_none() {
        return Err(JsonNeedsOut.into());
    }
    let bytes = fs::read(&args.file).map_err(|source| UnreadableInput {
        path: args.file.clone(),
        source,
    })?;
    let xml =
        cryxml::read(&bytes).with_context(|| format!("cannot convert {}", args.file.display()))?;
    let Some(out) = &args.out else {
        return Ok(xml.write_to(BufWriter::new(io::stdout().lock()))?);
    };
    write_file(&xml, out)?;
    let summary = Summary {
        input: json::path(&args.file),
        output: json::path(out),
        format: match xml.format() {
            Format::Text => "text",
            Format::CryXml(signature) => signature.as_str(),
        },
        byte_order: match &xml {
            Xml::Text(_) => None,
            Xml::CryXml(document) => Some(match document.byte_order() {
                cryxml::ByteOrder::Little => "little",
                cryxml::ByteOrder::Big => "big",
            }),
        },
    };
    if as_json {
        json::print(&summary)?;
    } else {
        let order = summary
            .byte_order
            .map_or_else(String::new, |order| format!(", {order}-endian"));
        writeln!(
            io::stdout().lock(),
            "converted {} ({}{order}) to {}",
            summary.input,
            summary.format,
            summary.output
        )?;
    }
    Ok(())
}

/// Writes the XML to a `.part` file next to `out` and renames it, so `out` is never half written.
fn write_file(xml: &Xml<'_>, out: &Path) -> Result<()> {
    crate::query_file::write_replacing(out, |writer| xml.write_to(writer))
        .with_context(|| format!("cannot write {}", out.display()))
}
