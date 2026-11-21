// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::UNIX_EPOCH;

use p4kx_extraction::{Archive, Build, DataForgeEntry, DataForgeOrigin, SourceEncryption};
use serde::Deserialize;

use super::TestResult;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Profile {
    schema_version: u8,
    pub(super) old: Side,
    pub(super) new: Side,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Side {
    pub(super) archive: PathBuf,
    archive_bytes: u64,
    modified_seconds: u64,
    modified_nanos: u32,
    build: Option<BuildIdentity>,
    pub(super) dataforge: DataForgeIdentity,
    pub(super) record: RecordIdentity,
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
pub(super) struct DataForgeIdentity {
    pub(super) entry_index: u32,
    pub(super) path: String,
    pub(super) record_offset: u64,
    pub(super) record_length: u64,
    pub(super) encrypted: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct RecordIdentity {
    pub(super) index: u32,
    pub(super) guid: String,
    pub(super) record_path: String,
    pub(super) entry_ordinal: u32,
    pub(super) weight_bits: u32,
    pub(super) optional_slot: u32,
    pub(super) dupe_limit: Option<i32>,
}

impl Profile {
    pub(super) fn read() -> TestResult<Self> {
        let path = env::var_os("P4KX_LOOT_DELTA_FIXTURE")
            .ok_or("private installed Loot profile is required")?;
        let bytes =
            fs::read(path).map_err(|_| "private installed Loot profile could not be read")?;
        let profile: Self = serde_json::from_slice(&bytes)
            .map_err(|_| "private installed Loot profile is invalid")?;
        if profile.schema_version != 2
            || profile.old.archive == profile.new.archive
            || profile.old.record.guid != profile.new.record.guid
            || profile.old.record.record_path != profile.new.record.record_path
            || profile.old.record.entry_ordinal != profile.new.record.entry_ordinal
            || profile.old.record.weight_bits == profile.new.record.weight_bits
            || profile.old.record.optional_slot != profile.new.record.optional_slot
            || profile.old.record.dupe_limit.is_none()
            || profile.new.record.dupe_limit.is_some()
        {
            return Err("private profile must describe one changed authored Loot field".into());
        }
        Ok(profile)
    }
}

impl Side {
    pub(super) fn check_archive(&self, archive: &Archive) -> TestResult {
        let modified = archive
            .file_modified()
            .and_then(|stamp| stamp.duration_since(UNIX_EPOCH).ok())
            .ok_or("Archive lacks a measured modification time")?;
        if archive.file_len() != self.archive_bytes
            || modified.as_secs() != self.modified_seconds
            || modified.subsec_nanos() != self.modified_nanos
        {
            return Err("Archive no longer matches the audited physical stamp".into());
        }
        match (&self.build, archive.build()) {
            (Some(expected), Some(build)) if expected.matches(build) => Ok(()),
            (None, None) => Ok(()),
            _ => Err("Archive Build does not match the audited manifest".into()),
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

impl DataForgeIdentity {
    pub(super) fn check_entry(&self, archive: &Archive, input: &DataForgeEntry) -> TestResult {
        let DataForgeOrigin::Archive(origin) = input.origin() else {
            return Err("selected DataForge is missing its original Archive proof".into());
        };
        let leaf = origin.entry();
        let range = leaf.metadata().record_range();
        let encryption = match origin.encryption() {
            SourceEncryption::Encrypted => Some(true),
            SourceEncryption::Unencrypted => Some(false),
            _ => None,
        };
        if !input.origin().matches_archive(archive)
            || origin.ancestors().len() != 0
            || leaf.ordinal() != self.entry_index
            || leaf.path() != self.path
            || range.offset() != self.record_offset
            || range.len() != self.record_length
            || encryption != self.encrypted
        {
            return Err("selected DataForge does not match the audited physical Entry".into());
        }
        Ok(())
    }
}
