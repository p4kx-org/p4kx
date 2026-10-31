// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The skin ports of every Entity Record of a real Archive: how many name no attachment helper.

#![expect(clippy::print_stdout, reason = "the counts are the output")]

use std::error::Error;

use p4kx_assembly::{ArchiveFiles, ENTITY_TYPE, LoadoutResolver, PortNode};
use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_testkit::{real_archive_key, real_archive_path};

type TestResult = Result<(), Box<dyn Error>>;

/// Returns whether the port is defined by its Entity as a skin attachment.
fn is_skin(port: &PortNode) -> bool {
    port.definition()
        .is_some_and(p4kx_assembly::PortDefinition::is_skin)
}

/// Returns whether the port definition gives no attachment helper.
fn lacks_helper(port: &PortNode) -> bool {
    port.definition()
        .is_some_and(|definition| definition.transform().is_none())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_skin_ports_without_a_helper_are_counted() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, real_archive_key().ok())?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let files = ArchiveFiles::new(&archive);
    let mut resolver = LoadoutResolver::new(&forge)
        .with_files(&files)
        .stopping_at_docked_vehicles();
    let mut skin_ports = 0_usize;
    let mut without_helper = Vec::new();
    for record in forge
        .records()
        .filter(|record| record.type_name().is_ok_and(|name| name == ENTITY_TYPE))
    {
        let Ok(tree) = resolver.resolve(&record) else {
            continue;
        };
        for port in tree.ports().iter().filter(|port| is_skin(port)) {
            skin_ports += 1;
            if lacks_helper(port) {
                without_helper.push(format!("{}/{}", tree.class_name(), port.name()));
            }
        }
    }
    println!(
        "{skin_ports} skin ports in the Entity Records, {} without a helper",
        without_helper.len()
    );
    for line in without_helper.iter().take(10) {
        println!("    {line}");
    }
    assert!(skin_ports > 0, "no skin port found");
    Ok(())
}
