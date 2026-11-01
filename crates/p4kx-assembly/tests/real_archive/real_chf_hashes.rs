// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The hashes of the CHF files of a real Archive follow the formulas documented in
//! `docs/chf-hashes.md`.

use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::io::Read;

use p4kx_assembly::character::{Chf, ItemPort, lowered_hash, name_hash, skin_attachment};
use p4kx_extraction::Archive;
use p4kx_testkit::{real_archive_key, real_archive_path};

use crate::real_paths::unified;

type TestResult = Result<(), Box<dyn Error>>;

const PORTS: [&str; 18] = [
    "head_itemport",
    "body_itemport",
    "eyes_itemport",
    "hair_itemport",
    "beard_itemport",
    "eyebrow_itemport",
    "stubble_itemport",
    "hat_itemport",
    "teeth_itemport",
    "eye_accessories_itemport",
    "eyelashes_itemport",
    "eyedetail_itemport",
    "skin_variant",
    "piercings_itemport",
    "piercings_nose_itemport",
    "piercings_eyebrows_itemport",
    "piercings_mouth_itemport",
    "universal_scalp_itemport",
];

fn collect(port: &ItemPort, into: &mut Vec<u32>) {
    into.push(port.port);
    for child in &port.children {
        collect(child, into);
    }
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
#[expect(
    clippy::print_stdout,
    clippy::too_many_lines,
    clippy::case_sensitive_file_extension_comparisons,
    reason = "the shares are the measurement; the path is lower-cased first"
)]
fn the_attachment_hashes_of_the_shipped_characters_resolve_to_item_port_names() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, real_archive_key().ok())?;
    let slots: HashMap<u32, &str> = PORTS
        .iter()
        .map(|port| (skin_attachment(port), *port))
        .collect();
    let ports: HashMap<u32, &str> = PORTS
        .iter()
        .map(|port| (lowered_hash(port), *port))
        .collect();
    let (mut files, mut materials, mut resolved) = (0_usize, 0_usize, 0_usize);
    let (mut port_nodes, mut named_ports) = (0_usize, 0_usize);
    let mut unresolved = HashSet::new();
    let mut unmatched_ports = HashSet::new();
    let mut default_materials = 0_usize;
    for entry in archive.entries() {
        let path = unified(entry.path());
        if !path.ends_with(".chf") {
            continue;
        }
        let mut bytes = Vec::new();
        archive.open_entry(entry)?.read_to_end(&mut bytes)?;
        let chf = Chf::read(&bytes)?;
        files += 1;
        let default =
            path.ends_with("/masculinedefault.chf") || path.ends_with("/femininedefault.chf");
        for material in &chf.character.materials {
            materials += 1;
            if slots.contains_key(&material.attachment) {
                resolved += 1;
            } else {
                unresolved.insert(material.attachment);
            }
            if default {
                default_materials += 1;
                assert!(
                    slots.contains_key(&material.attachment),
                    "{path}: attachment {:#010x} names no known item port",
                    material.attachment
                );
            }
        }
        let mut hashes = Vec::new();
        if let Some(root) = &chf.character.item_ports {
            collect(root, &mut hashes);
        }
        port_nodes += hashes.len();
        for hash in &hashes {
            if ports.contains_key(hash) {
                named_ports += 1;
            } else {
                unmatched_ports.insert(*hash);
            }
        }
        if default {
            let dna = chf.character.dna.as_ref();
            assert!(dna.is_some(), "{path}: no face DNA");
        }
        if default && let Some(dna) = &chf.character.dna {
            let gender = if path.ends_with("/masculinedefault.chf") {
                "male"
            } else {
                "female"
            };
            let rig = name_hash(&format!("protos_human_{gender}_face_t1_pu"));
            assert_eq!(dna.gender_hash, rig, "{path}");
        }
    }
    println!(
        "{files} CHF files: {resolved} of {materials} attachment hashes resolve to a known item \
         port by the CRC-32 formula ({} distinct unresolved: {unresolved:08x?}); {named_ports} of \
         {port_nodes} item port hashes match a known name by CRC-32C ({} distinct unmatched: \
         {unmatched_ports:08x?}); {default_materials} attachments in the two default characters, \
         all resolved",
        unresolved.len(),
        unmatched_ports.len()
    );
    assert!(
        files > 0 && default_materials > 0,
        "the Archive holds default CHF files"
    );
    Ok(())
}
