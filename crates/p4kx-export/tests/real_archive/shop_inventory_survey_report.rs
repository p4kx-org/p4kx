// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Write as _;

use p4kx_extraction::Archive;

use super::{Relations, Survey, TestResult};

pub(super) fn report(
    archive: &Archive,
    files: usize,
    relations: &Relations,
    survey: &Survey,
) -> TestResult {
    let build = archive.build().ok_or("actual Build identity")?;
    let unlinked = files
        .checked_sub(relations.referenced.len())
        .ok_or("observed inventory counts")?;
    let mut out = std::io::stderr().lock();
    writeln!(
        out,
        "Shop inventory oracle for {} {} {} {}: {files} physical files, {} resolved, {} placeholders, {unlinked} without a physical relation reference",
        build.branch(),
        build.version(),
        build.change_number(),
        build.build_id(),
        relations.resolved.len(),
        relations.placeholder.len()
    )?;
    writeln!(
        out,
        "Observed inventory source totals {survey:?}, shop statuses {:?}",
        relations.statuses
    )?;
    Ok(())
}
