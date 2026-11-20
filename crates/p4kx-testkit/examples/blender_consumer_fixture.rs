// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Writes synthetic Archives for source-built package consumer verification in Blender.

use std::error::Error;
use std::path::PathBuf;

use p4kx_testkit::{
    ArchiveBuilder, TestEntry, assembly_entries, livery_entries, mounted_overlay_entries, test_key,
};

#[path = "blender_consumer_fixture/character.rs"]
mod character;

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
    let cases = [
        Case {
            name: "assembly",
            entries: assembly_entries,
        },
        Case {
            name: "livery",
            entries: livery_entries,
        },
        Case {
            name: "character",
            entries: character::entries,
        },
    ];
    let worn_helper = std::env::var_os("P4KX_TEST_WORN_HELPER_FIXTURE").map(|_| Case {
        name: "worn-helper",
        entries: mounted_overlay_entries,
    });
    for case in cases.into_iter().chain(worn_helper) {
        (case.entries)()
            .into_iter()
            .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
            .write_to(output.join(format!("{}.p4k", case.name)))?;
    }
    character::write_chf(&output)?;
    std::fs::write(output.join("synthetic.key"), test_key().to_hex())?;
    Ok(())
}
