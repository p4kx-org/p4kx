// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment::resource_topology::read_resource_topology_placements;
use p4kx_extraction::ArchiveWatch;

use super::{TestResult, check_selection, open_archive, receipt};
use crate::real_archive_verification::print_line;

#[test]
#[ignore = "real Archive: configure the private Build-bound topology receipt and P4KX_TOPOLOGY_SAMPLE"]
fn installed_topology_sdk_reads_one_verified_root() -> TestResult {
    let archive = open_archive()?;
    let watch = ArchiveWatch::new(&archive);
    let sample = receipt::Report::sample(&archive)?;
    let result = read_resource_topology_placements(&archive, sample.record_index, None)
        .map_err(|_| "cold scoped topology query failed")?;
    check_selection(&archive, sample.record_index, &result)?;
    sample.verify(&result)?;
    watch
        .check()
        .map_err(|_| "Archive changed during cold topology verification")?;
    print_line("Scoped topology SDK: a single cold query reproduced its verified sample");
    Ok(())
}
