// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, BufWriter, Write};

use anyhow::Result;
use p4kx_assembly::character::ChfObservedChange;
use p4kx_extraction::{
    ArchiveWriteProtection, CancelToken, DecimalU64, FieldOrigin, FieldPath, OriginArtifactBinding,
    OriginDigestWriter, OriginOutputAddress, write_origin_companion,
};

use crate::chf_diff_origins::DiffFieldOrigins;
use crate::chf_diff_text::{self, TextField, TextSite};
use crate::cli::ChfDiffArgs;
use crate::file_receipt::StdoutReceipt;

pub(crate) fn write(
    args: &ChfDiffArgs,
    changes: &[ChfObservedChange],
    origins: &DiffFieldOrigins,
) -> Result<()> {
    let sources = [
        Some(args.a.as_path()),
        Some(args.b.as_path()),
        args.archive.as_deref(),
    ];
    let sources: Vec<_> = sources.into_iter().flatten().collect();
    let cancel = CancelToken::new();
    let protection = args
        .origin_out
        .as_ref()
        .map(|_| ArchiveWriteProtection::for_sources(&sources, cancel.flag()))
        .transpose()?;
    let durable = args
        .origin_out
        .as_deref()
        .map(|path| StdoutReceipt::new(path, protection.as_ref()))
        .transpose()?;
    let mut stream = OriginDigestWriter::new(BufWriter::new(io::stdout().lock()));
    let mut fields = Vec::new();
    chf_diff_text::write(
        &mut stream,
        changes.iter().map(ChfObservedChange::change),
        |site| {
            fields.try_reserve(1).map_err(io::Error::other)?;
            fields.push(project(&site, origins)?);
            Ok(())
        },
    )?;
    let binding = OriginArtifactBinding::Stdout {
        digest: stream.finish()?,
    };
    let write = |writer: &mut dyn Write| {
        write_origin_companion(
            writer,
            &binding,
            &origins.provenance,
            fields.iter().map(|(address, origin)| (address, *origin)),
        )
    };
    if let Some(durable) = durable {
        durable.publish(write)?;
    }
    if args.origin_stderr {
        write(&mut BufWriter::new(io::stderr().lock()))?;
    }
    Ok(())
}

fn project<'origin>(
    site: &TextSite,
    origins: &'origin DiffFieldOrigins,
) -> io::Result<(OriginOutputAddress, &'origin FieldOrigin)> {
    let path = match site.field {
        TextField::Identical => "/identical".to_owned(),
        TextField::Path(row) => format!("/changes/{row}/path"),
        TextField::Old(row) => format!("/changes/{row}/old"),
        TextField::New(row) => format!("/changes/{row}/new"),
        TextField::Disposition(row) => format!("/changes/{row}/status"),
    };
    let field = FieldPath::parse(&path).map_err(io::Error::other)?;
    let origin = match site.field {
        TextField::Disposition(row) => origins.disposition(row),
        _ => origins.field_origins.get(&field),
    }
    .ok_or_else(|| io::Error::other("missing CHF text field origin"))?;
    Ok((
        OriginOutputAddress::TextBytes {
            start: DecimalU64::new(site.range.start),
            end_exclusive: DecimalU64::new(site.range.end),
            field,
        },
        origin,
    ))
}
