// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use p4kx_assembly::SocArchiveMember;
use p4kx_extraction::cryxml::{SourceDocument, SourceReadOptions};
use p4kx_extraction::{
    Archive, ArchiveWatch, Build, Entry, InputScope, PhysicalSourceKind, SourceUseRole,
};
use p4kx_testkit::{real_archive_key, watchdog};
use serde::Deserialize;

use super::{
    LocationPhysicsDescriptorSelection, LocationPhysicsDescriptorSnapshot,
    LocationPhysicsDescriptorStatus, read_location_physics_descriptor,
};

#[path = "location_physics_installed_stage.rs"]
mod staged;
#[path = "location_physics_installed_xml.rs"]
mod xml;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Profile {
    schema_version: u8,
    old: Side,
    current: Side,
    cargo_target: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Side {
    archive: PathBuf,
    archive_bytes: u64,
    modified_seconds: u64,
    modified_nanos: u32,
    build: Option<BuildIdentity>,
    descriptor: Member,
    cargo: Member,
    cargo_chunk: usize,
    descriptor_encrypted: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BuildIdentity {
    branch: String,
    version: String,
    change_number: String,
    build_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Member {
    outer: PhysicalEntry,
    leaf: PhysicalEntry,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PhysicalEntry {
    index: u32,
    path: String,
    record_offset: u64,
    record_length: u64,
}

#[test]
#[ignore = "real Archive: set P4KX_LOCATION_GRID_DELTA_FIXTURE and pass --ignored"]
fn installed_location_grid_descriptor_delta_retains_cargo_relationship() -> TestResult {
    watchdog::start();
    let path = env::var_os("P4KX_LOCATION_GRID_DELTA_FIXTURE").ok_or("private profile required")?;
    let path = Path::new(&path);
    let profile = Profile::read(path)?;
    let scratch = tempfile::tempdir_in(path.parent().ok_or("private profile folder required")?)?;
    check_side(&profile.old, true, &profile.cargo_target, scratch.path())?;
    check_side(
        &profile.current,
        false,
        &profile.cargo_target,
        scratch.path(),
    )
}

impl Profile {
    fn read(path: &Path) -> TestResult<Self> {
        let bytes = fs::read(path).map_err(|_| "private profile cannot be read")?;
        let profile: Self =
            serde_json::from_slice(&bytes).map_err(|_| "private profile is invalid")?;
        if profile.schema_version != 1
            || profile.old.archive == profile.current.archive
            || profile.old.descriptor.outer.path != profile.current.descriptor.outer.path
            || profile.old.descriptor.leaf.path != profile.current.descriptor.leaf.path
            || profile.old.cargo.outer.path != profile.current.cargo.outer.path
            || profile.old.cargo.leaf.path != profile.current.cargo.leaf.path
            || profile.current.build.is_none()
            || profile.cargo_target.is_empty()
        {
            return Err(
                "private profile must bind both original descriptors and cargo members".into(),
            );
        }
        Ok(profile)
    }
}

fn check_side(side: &Side, grid_expected: bool, cargo_target: &str, scratch: &Path) -> TestResult {
    let key = real_archive_key().map_err(|_| "installed Archive key unavailable")?;
    let archive = Archive::open_with_key(&side.archive, Some(key))
        .map_err(|_| "audited Archive unavailable")?;
    side.check_archive(&archive)?;
    let watch = ArchiveWatch::new(&archive);
    watch.check().map_err(|_| "audited Archive changed")?;
    let selection = LocationPhysicsDescriptorSelection::new(
        side.descriptor.outer.index,
        side.descriptor.leaf.index,
    );
    let snapshot = read_location_physics_descriptor(&archive, selection, None)
        .map_err(|_| "physical location-grid SDK query failed")?;
    check_descriptor(&snapshot, side, grid_expected)?;
    let member = side.descriptor.read(&archive)?;
    let bytes = member
        .payload()
        .map_err(|_| "audited descriptor cannot be read")?;
    let original = SourceDocument::parse(bytes, SourceReadOptions::default())
        .map_err(|_| "audited descriptor XML cannot be read")?;
    if original.is_lossy() || original.root().name() != "ObjectContainer" {
        return Err("audited descriptor XML is not a lossless ObjectContainer".into());
    }
    xml::check_grid(original.root(), snapshot.grid.as_deref(), grid_expected)?;
    staged::compare(&archive, side.descriptor.outer.index, &snapshot, scratch)?;
    let cargo = side.cargo.read(&archive)?;
    xml::check_cargo(
        cargo
            .payload()
            .map_err(|_| "audited cargo stream cannot be read")?,
        side.cargo_chunk,
        cargo_target,
    )?;
    watch
        .check()
        .map_err(|_| "audited Archive changed during the witness")?;
    Ok(())
}

fn check_descriptor(
    snapshot: &LocationPhysicsDescriptorSnapshot,
    side: &Side,
    grid_expected: bool,
) -> TestResult {
    if snapshot.status != LocationPhysicsDescriptorStatus::Read
        || snapshot.present.value() != Some(&grid_expected)
        || snapshot.present.origin().contains_encrypted_source() != side.descriptor_encrypted
        || !snapshot.present.origin().source_set_complete()
    {
        return Err(
            "SDK did not prove the expected grid presence with its original evidence".into(),
        );
    }
    let reference = snapshot
        .descriptor
        .as_ref()
        .and_then(p4kx_extraction::SourcedValue::value)
        .ok_or("SDK did not retain the original descriptor identity")?;
    let origin = snapshot.present.origin();
    let inventory = &snapshot.source_inventory;
    let matching_payload = inventory.iter_uses().any(|used| {
        used.source() == reference.input_ref()
            && used.role() == SourceUseRole::Selector
            && used.scope() == InputScope::Payload
            && origin.source_uses_refs().contains(used.id())
    });
    if reference.entry_index() != side.descriptor.leaf.index || !matching_payload {
        return Err("SDK presence does not use the audited physical descriptor bytes".into());
    }
    let leaf = inventory
        .iter_sources()
        .find(|source| source.id() == reference.input_ref())
        .ok_or("SDK descriptor source missing from its inventory")?;
    let [owner_ref] = leaf.ancestors() else {
        return Err("SDK descriptor has unexpected physical ancestry".into());
    };
    let owner = inventory
        .iter_sources()
        .find(|source| source.id() == owner_ref)
        .ok_or("SDK descriptor owner missing from its inventory")?;
    if !side.descriptor.outer.matches_source(owner.kind())
        || !side.descriptor.leaf.matches_source(leaf.kind())
    {
        return Err("SDK selected a different physical source from the audited descriptor".into());
    }
    Ok(())
}

impl Side {
    fn check_archive(&self, archive: &Archive) -> TestResult {
        let modified = archive
            .file_modified()
            .and_then(|stamp| stamp.duration_since(UNIX_EPOCH).ok())
            .ok_or("Archive modification time is unavailable")?;
        if archive.file_len() != self.archive_bytes
            || modified.as_secs() != self.modified_seconds
            || modified.subsec_nanos() != self.modified_nanos
        {
            return Err("Archive did not match the recorded physical stamp".into());
        }
        match (&self.build, archive.build()) {
            (None, None) => Ok(()),
            (Some(expected), Some(build)) if expected.matches(build) => Ok(()),
            _ => Err("Archive did not match the recorded Build metadata".into()),
        }
    }
}

impl BuildIdentity {
    fn matches(&self, build: &Build) -> bool {
        self.branch == build.branch()
            && self.version == build.version()
            && self.change_number == build.change_number()
            && self.build_id == build.build_id()
    }
}

impl Member {
    fn read(&self, archive: &Archive) -> TestResult<SocArchiveMember> {
        let outer = self.outer.select(archive)?;
        let container = archive
            .open_container(outer)
            .map_err(|_| "audited source container could not open")?;
        let leaf = self.leaf.select(&container)?;
        SocArchiveMember::read(&container, leaf, || Ok(()))
            .map_err(|_| "audited source member could not be verified".into())
    }
}

impl PhysicalEntry {
    fn select<'archive>(&self, archive: &'archive Archive) -> TestResult<&'archive Entry> {
        let index = usize::try_from(self.index)?;
        let entry = archive
            .entries()
            .get(index)
            .ok_or("audited Entry is absent")?;
        let range = entry.metadata().record_range();
        if entry.index() != index
            || entry.path() != self.path
            || range.offset() != self.record_offset
            || range.len() != self.record_length
        {
            return Err("audited Entry directory identity differs".into());
        }
        Ok(entry)
    }

    fn matches_source(&self, source: &PhysicalSourceKind) -> bool {
        matches!(source, PhysicalSourceKind::ArchiveEntry(entry)
            if u32::try_from(entry.entry_index()).ok() == Some(self.index)
                && entry.path().value() == Some(&self.path)
                && entry.record_range().offset.get() == self.record_offset
                && entry.record_range().length.get() == self.record_length)
    }
}
