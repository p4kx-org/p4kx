// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ffi::OsStr;
use std::io::Read;
use std::path::PathBuf;

use p4kx_extraction::{Archive, ArchiveWatch};
use serde_json::Value;

use crate::package_fault::{self, Check};
use crate::package_support::{self, Built, Measured, Scratch, TestResult};

const CHF: &str = "masculinedefault.chf";
const LOADOUT: &str =
    "Scripts/Loadouts/Character/Human/Armor/NPC/Criminal/Pyro/Criminal_Gang_Guard_Heavy_Boss.xml";
const MAX_CHARACTER_BYTES: u64 = 256 << 20;

pub(crate) fn verify(scratch: &Scratch) -> TestResult {
    let archive = Archive::open_with_key(
        p4kx_testkit::real_archive_path()?,
        Some(p4kx_testkit::real_archive_key()?),
    )?;
    let watch = ArchiveWatch::new(&archive);
    watch.check()?;
    let chf = extract_chf(scratch, &archive)?;
    let built = package_support::build(
        scratch,
        "character",
        None,
        "character",
        &[
            OsStr::new("--chf"),
            chf.as_os_str(),
            OsStr::new("--loadout"),
            OsStr::new(LOADOUT),
            OsStr::new("--anim"),
            OsStr::new("ai_civ_stand_ntrl_tray_2h_carry_idle_01"),
            OsStr::new("--anim"),
            OsStr::new("ai_cryopod_sleep_idle_01"),
        ],
    );
    let result = built.and_then(|built| {
        let measured = package_support::verify(&built)?;
        check(&built, &measured)
    });
    watch.check()?;
    result
}

fn extract_chf(scratch: &Scratch, archive: &Archive) -> package_fault::Result<PathBuf> {
    let entries: Vec<_> = archive
        .entries()
        .iter()
        .filter(|entry| {
            entry
                .path()
                .to_ascii_lowercase()
                .strip_suffix(CHF)
                .is_some_and(|rest| rest.ends_with(['/', '\\']))
        })
        .collect();
    package_fault::require(entries.len() == 1, Check::SourceIdentity)?;
    let entry = entries.first().ok_or(Check::SourceIdentity)?;
    let mut bytes = Vec::new();
    archive.open_entry(entry)?.read_to_end(&mut bytes)?;
    let file = scratch.path().join(CHF);
    std::fs::write(&file, bytes)?;
    Ok(file)
}

fn check(built: &Built, measured: &Measured) -> TestResult {
    package_fault::require(measured.meshes == 1, Check::MinimumMeshes)?;
    package_fault::require(
        package_support::nodes_of(&built.manifest, "character")? == 1,
        Check::CharacterRig,
    )?;
    package_fault::require(
        built
            .manifest
            .get("textures")
            .and_then(Value::as_array)
            .map(Vec::len)
            == Some(0),
        Check::Textures,
    )?;
    let rig = built
        .manifest
        .get("meshes")
        .and_then(Value::as_array)
        .and_then(|meshes| meshes.first())
        .and_then(|mesh| mesh.get("rig"))
        .ok_or(Check::CharacterRig)?;
    let count = |key: &str| {
        rig.get(key)
            .and_then(Value::as_u64)
            .ok_or(Check::CharacterRig)
    };
    package_fault::require(
        count("bones")? > 50 && count("skinned_meshes")? > 1 && count("morph_targets")? > 0,
        Check::CharacterRig,
    )?;
    package_fault::require(
        rig.get("animations")
            .and_then(Value::as_array)
            .map(Vec::len)
            == Some(2),
        Check::CharacterAnimations,
    )?;
    let parts = rig
        .get("parts")
        .and_then(Value::as_array)
        .ok_or(Check::CharacterParts)?;
    package_fault::require(
        parts
            .first()
            .is_some_and(|part| part.get("origin").and_then(Value::as_str) == Some("body"))
            && parts.len() > 1,
        Check::CharacterParts,
    )?;
    package_fault::require(measured.bytes <= MAX_CHARACTER_BYTES, Check::PackageSize)?;
    report(rig);
    Ok(())
}

#[expect(
    clippy::print_stdout,
    reason = "only aggregate rig measurements are reported"
)]
fn report(rig: &Value) {
    println!(
        "character totals: bones={} skinned_meshes={} morph_targets={} parts={} animations={}",
        rig["bones"].as_u64().unwrap_or_default(),
        rig["skinned_meshes"].as_u64().unwrap_or_default(),
        rig["morph_targets"].as_u64().unwrap_or_default(),
        rig["parts"].as_array().map_or(0, Vec::len),
        rig["animations"].as_array().map_or(0, Vec::len)
    );
}
