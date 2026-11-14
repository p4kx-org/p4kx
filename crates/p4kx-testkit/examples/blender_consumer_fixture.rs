// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Writes synthetic Archives for source-built package consumer verification in Blender.

use std::error::Error;
use std::path::PathBuf;

use p4kx_testkit::{ArchiveBuilder, TestEntry, assembly_entries, livery_entries, test_key};

struct Case {
    name: &'static str,
    entries: fn() -> Vec<TestEntry>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("provide an absolute new fixture directory")?;
    if !output.is_absolute() || output.exists() {
        return Err("fixture directory must be absolute and new".into());
    }
    std::fs::create_dir_all(&output)?;
    for case in [
        Case {
            name: "assembly",
            entries: assembly_entries,
        },
        Case {
            name: "livery",
            entries: livery_entries,
        },
    ] {
        (case.entries)()
            .into_iter()
            .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
            .write_to(output.join(format!("{}.p4k", case.name)))?;
    }
    std::fs::write(output.join("synthetic.key"), test_key().to_hex())?;
    Ok(())
}
