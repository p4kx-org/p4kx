// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::PathBuf;
use std::time::SystemTime;

use p4kx_extraction::{Archive, ArchiveWatch, Build, Guid, Language};

use crate::commodity_v3_fixture::TestResult;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Dashboard {
    pub(super) blueprint: Guid,
    pub(super) location_disabled: bool,
    pub(super) first_port: String,
    pub(super) reward_link_encrypted: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Identity {
    pub(super) build: Build,
    pub(super) language: Language,
    source_path: PathBuf,
    archive_bytes: u64,
    archive_modified: Option<SystemTime>,
}

impl Identity {
    fn read(archive: &Archive, language: Language) -> TestResult<Self> {
        Ok(Self {
            build: archive.build().ok_or("synthetic Archive Build")?.clone(),
            language,
            source_path: archive.path().to_owned(),
            archive_bytes: archive.file_len(),
            archive_modified: archive.file_modified(),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Published {
    pub(super) identity: Identity,
    pub(super) dashboard: Dashboard,
}

pub(super) fn refresh(
    cache: &mut Option<Published>,
    archive: &Archive,
    language: Language,
    collect: impl FnOnce() -> TestResult<Dashboard>,
    replace: impl FnOnce(&Published) -> TestResult,
) -> TestResult {
    let watch = ArchiveWatch::new(archive);
    watch.check()?;
    let identity = Identity::read(archive, language)?;
    if cache
        .as_ref()
        .is_some_and(|previous| previous.identity == identity)
    {
        return Ok(());
    }
    let candidate = Published {
        identity,
        dashboard: collect()?,
    };
    watch.check()?;
    replace(&candidate)?;
    *cache = Some(candidate);
    Ok(())
}
