// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use p4kx_extraction::cryxml::{self, Format, Xml};
use p4kx_extraction::{
    ArchiveWriteProtection, CancelToken, FieldOrigins, ObservedFileRead, OriginArtifactBinding,
    OriginDigestWriter, SourceInventory, read_observed_file,
};
use serde::Serialize;
use thiserror::Error;

use crate::cli_cryxml::CryXmlArgs;
use crate::cryxml_origins::CryXmlOrigins;
use crate::file_receipt::{FileReceipt, StdoutReceipt};
use crate::json;

/// The input file of `p4kx cryxml` cannot be read.
#[derive(Debug, Error)]
#[error("cannot read {}", path.display())]
pub(crate) struct UnreadableInput {
    path: PathBuf,
    #[source]
    source: io::Error,
}

/// XML and its JSON summary both need stdout.
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonSummary {
    #[serde(flatten)]
    summary: Summary,
    origin_receipt: String,
    field_origins: FieldOrigins,
    provenance: SourceInventory,
}

/// Converts a CryXml file to XML on stdout or in `--out`.
pub(crate) fn run(args: &CryXmlArgs, as_json: bool) -> Result<()> {
    if as_json && args.out.is_none() {
        return Err(JsonNeedsOut.into());
    }
    let input = read_observed_file(&args.file).map_err(|source| UnreadableInput {
        path: args.file.clone(),
        source,
    })?;
    let xml = cryxml::read(input.bytes())
        .with_context(|| format!("cannot convert {}", args.file.display()))?;
    args.out.as_deref().map_or_else(
        || write_stdout(args, &input, &xml),
        |out| {
            write_file(args, out, &input, &xml, as_json)
                .with_context(|| format!("cannot write {}", out.display()))
        },
    )
}

fn write_stdout(args: &CryXmlArgs, input: &ObservedFileRead, xml: &Xml<'_>) -> Result<()> {
    if !args.origin_stderr && args.origin_out.is_none() {
        return Ok(xml.write_to(BufWriter::new(io::stdout().lock()))?);
    }
    let protection = args
        .origin_out
        .as_ref()
        .map(|_| ArchiveWriteProtection::new(&args.file, CancelToken::new().flag()))
        .transpose()?;
    let durable = args
        .origin_out
        .as_deref()
        .map(|path| StdoutReceipt::new(path, protection.as_ref()))
        .transpose()?;
    let origins = CryXmlOrigins::new(input)?;
    let mut stdout = OriginDigestWriter::new(BufWriter::new(io::stdout().lock()));
    let receipt = origins.render(xml, &mut stdout)?;
    let binding = OriginArtifactBinding::Stdout {
        digest: stdout.finish()?,
    };
    if let Some(durable) = durable {
        durable.publish(|writer| receipt.write(writer, &binding))?;
    }
    if args.origin_stderr {
        receipt.write(&mut BufWriter::new(io::stderr().lock()), &binding)?;
    }
    Ok(())
}

fn write_file(
    args: &CryXmlArgs,
    out: &Path,
    input: &ObservedFileRead,
    xml: &Xml<'_>,
    as_json: bool,
) -> Result<()> {
    let protection = ArchiveWriteProtection::new(&args.file, CancelToken::new().flag())?;
    let output = FileReceipt::new(out, None, Some(&protection))?;
    let origins = CryXmlOrigins::new(input)?;
    output.publish(
        |writer| origins.render(xml, writer),
        |writer, binding, receipt| receipt.write(writer, binding),
    )?;
    let summary = Summary::new(&args.file, out, xml);
    if as_json {
        let receipt_name = output
            .path()
            .file_name()
            .ok_or_else(|| io::Error::other("missing origin companion basename"))?;
        let (field_origins, provenance) = CryXmlOrigins::new(input)?.summary(xml)?;
        json::print(&JsonSummary {
            summary,
            origin_receipt: json::path(Path::new(receipt_name)),
            field_origins,
            provenance,
        })?;
    } else {
        summary.print()?;
    }
    Ok(())
}

impl Summary {
    fn new(input: &Path, output: &Path, xml: &Xml<'_>) -> Self {
        Self {
            input: json::path(input),
            output: json::path(output),
            format: match xml.format() {
                Format::Text => "text",
                Format::CryXml(signature) => signature.as_str(),
            },
            byte_order: match xml {
                Xml::Text(_) => None,
                Xml::CryXml(document) => Some(match document.byte_order() {
                    cryxml::ByteOrder::Little => "little",
                    cryxml::ByteOrder::Big => "big",
                }),
            },
        }
    }

    fn print(&self) -> io::Result<()> {
        let order = self
            .byte_order
            .map_or_else(String::new, |order| format!(", {order}-endian"));
        writeln!(
            io::stdout().lock(),
            "converted {} ({}{order}) to {}",
            self.input,
            self.format,
            self.output
        )
    }
}
