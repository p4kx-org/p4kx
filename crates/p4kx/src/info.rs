// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};

use anyhow::Result;
use p4kx_channels::channel_name;
use p4kx_extraction::{Archive, ArchiveLayoutInfo, Build, Entry};
use serde::Serialize;

use crate::cli::InfoArgs;
use crate::json::{self, BuildJson};

const UNKNOWN: &str = "unknown";

#[derive(Serialize)]
struct Summary<'a> {
    archive: String,
    channel: Option<String>,
    build: Option<BuildJson<'a>>,
    metadata: ArchiveLayoutInfo,
    entry_count: usize,
    encrypted_count: usize,
    issue_count: usize,
    compressed_size: u64,
    size: u64,
}

/// Prints the Channel, the Build and a summary of the Archive, one `Label: value` line each.
pub(crate) fn run(args: &InfoArgs, as_json: bool) -> Result<()> {
    let archive = crate::key::open(&args.archive)?;
    if archive.build().is_none() {
        warn_about_manifest(&archive)?;
    }
    crate::issues::warn(&archive)?;
    let patches = crate::launcher_discovery::patches()?;
    let build = archive.build().map(|build| patches.attach(build));
    let entries = archive.entries();
    let info = Summary {
        archive: json::path(archive.path()),
        channel: channel_name(archive.path()),
        build: build.as_ref().map(BuildJson::from),
        metadata: ArchiveLayoutInfo::of(archive.metadata()),
        entry_count: entries.len(),
        encrypted_count: entries.iter().filter(|entry| entry.is_encrypted()).count(),
        issue_count: archive.issue_count(),
        compressed_size: entries.iter().map(Entry::compressed_size).sum(),
        size: entries.iter().map(Entry::uncompressed_size).sum(),
    };
    if as_json {
        json::print(&info)?;
    } else {
        print_lines(&info, build.as_ref())?;
    }
    Ok(())
}

fn print_lines(info: &Summary<'_>, build: Option<&Build>) -> io::Result<()> {
    let mut lines: Vec<(&str, String)> = vec![
        ("Archive", info.archive.clone()),
        (
            "Channel",
            info.channel.clone().unwrap_or_else(|| UNKNOWN.to_owned()),
        ),
    ];
    if let Some(build) = build {
        lines.extend([
            ("Branch", build.branch().to_owned()),
            ("Version", build.version().to_owned()),
            (
                "Public patch",
                build.public_patch().unwrap_or(UNKNOWN).to_owned(),
            ),
            ("Change number", build.change_number().to_owned()),
            ("BuildId", build.build_id().to_owned()),
        ]);
    } else {
        lines.push(("Build", UNKNOWN.to_owned()));
    }
    lines.extend([
        ("Entries", info.entry_count.to_string()),
        ("Encrypted", info.encrypted_count.to_string()),
        ("Issues", info.issue_count.to_string()),
        ("Compressed size", info.compressed_size.to_string()),
        ("Size", info.size.to_string()),
    ]);
    let mut out = io::stdout().lock();
    for (label, value) in lines {
        writeln!(out, "{:<16} {value}", format!("{label}:"))?;
    }
    Ok(())
}

fn warn_about_manifest(archive: &Archive) -> io::Result<()> {
    let Some(folder) = archive.path().parent() else {
        return Ok(());
    };
    if let Err(error) = Build::read_from(folder) {
        let cause = std::error::Error::source(&error).map(ToString::to_string);
        let mut err = io::stderr().lock();
        match cause {
            Some(cause) => writeln!(err, "warning: {error}: {cause}")?,
            None => writeln!(err, "warning: {error}")?,
        }
    }
    Ok(())
}
