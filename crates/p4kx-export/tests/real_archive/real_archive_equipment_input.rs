// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::env;

use p4kx_extraction::{Archive, ArchiveWatch, DataForgeEntry};
use p4kx_testkit::{real_archive_key, real_archive_path};

use crate::real_archive_catalog_table_parity::TestResult;

pub(super) fn with_archive(check: impl FnOnce(&Archive) -> TestResult) -> TestResult {
    let path = real_archive_path().map_err(|_| "configure the installed Archive path")?;
    let key = real_archive_key().map_err(|_| "configure the trusted installed Archive key")?;
    let archive =
        Archive::open_with_key(path, Some(key)).map_err(|_| "open the installed Archive")?;
    let expected =
        env::var("P4KX_NEW_SDK_BUILD").map_err(|_| "configure the expected Build change number")?;
    let build = archive
        .build()
        .ok_or("the installed Archive lacks a Build")?;
    if build.change_number() != expected
        || expected.is_empty()
        || !expected.bytes().all(|byte| byte.is_ascii_digit())
        || [build.branch(), build.version(), build.build_id()]
            .iter()
            .any(|value| value.trim().is_empty())
    {
        return Err("the installed Archive differs from the configured Build".into());
    }
    let watch = ArchiveWatch::new(&archive);
    watch
        .check()
        .map_err(|_| "verify the original Archive stamp")?;
    let result = check(&archive);
    watch
        .check()
        .map_err(|_| "the original Archive changed during proof")?;
    result
}

pub(super) fn dataforge(archive: &Archive) -> TestResult<DataForgeEntry> {
    DataForgeEntry::read(archive).map_err(|_| "read the bound DataForge Entry".into())
}

pub(super) fn record_index(variable: &str) -> TestResult<u32> {
    env::var(variable)
        .ok()
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| "configure a private physical Ship Record index".into())
}
