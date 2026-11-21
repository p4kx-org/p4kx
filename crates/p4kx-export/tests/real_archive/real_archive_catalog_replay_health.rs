// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::Kind;
use serde::Deserialize;

use super::TestResult;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct KindReport {
    kind: String,
    source_records: Records,
    source_entries: Entries,
    written_rows: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Records {
    selected: u64,
    projected: u64,
    intentionally_skipped: u64,
    omitted_after_read_failure: u64,
    omitted_after_projection_failure: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Entries {
    selected: u64,
    projected: u64,
    intentionally_skipped: u64,
    failed_required_reads: u64,
    failed_projections: u64,
}

pub(super) fn check(
    kinds: &[KindReport],
    selected_kind_records: u64,
    written_rows: u64,
) -> TestResult {
    if kinds.len() != Kind::ALL.len() {
        return Err("catalog kind health does not contain exactly the registered Kinds".into());
    }
    let mut selected_total = 0_u64;
    let mut written_total = 0_u64;
    for (row, kind) in kinds.iter().zip(Kind::ALL) {
        if row.kind != kind.name() {
            return Err(format!("catalog kind health expected {kind}, found {}", row.kind).into());
        }
        reconcile(
            kind,
            "sourceRecords",
            row.source_records.selected,
            [
                row.source_records.projected,
                row.source_records.intentionally_skipped,
                row.source_records.omitted_after_read_failure,
                row.source_records.omitted_after_projection_failure,
            ],
        )?;
        reconcile(
            kind,
            "sourceEntries",
            row.source_entries.selected,
            [
                row.source_entries.projected,
                row.source_entries.intentionally_skipped,
                row.source_entries.failed_required_reads,
                row.source_entries.failed_projections,
            ],
        )?;
        selected_total = selected_total
            .checked_add(row.source_records.selected)
            .ok_or("selectedKindRecords total overflow")?;
        written_total = written_total
            .checked_add(row.written_rows)
            .ok_or("writtenRows total overflow")?;
    }
    if selected_total != selected_kind_records {
        return Err("catalog selectedKindRecords total does not reconcile".into());
    }
    if written_total != written_rows {
        return Err("catalog writtenRows total does not reconcile".into());
    }
    Ok(())
}

fn reconcile(kind: Kind, unit: &str, selected: u64, outcomes: [u64; 4]) -> TestResult {
    if outcomes.into_iter().try_fold(0_u64, u64::checked_add) != Some(selected) {
        return Err(format!("catalog kind health {kind} {unit} does not reconcile").into());
    }
    Ok(())
}
