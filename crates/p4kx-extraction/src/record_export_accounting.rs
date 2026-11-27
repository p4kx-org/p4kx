// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::Record;

use super::RecordOutcome;
use crate::{RecordExportReport, RecordFailure};

pub(super) fn summarise(
    records: &[Record<'_>],
    outcomes: Vec<(usize, RecordOutcome)>,
    written_record_indices: Option<Vec<u32>>,
) -> RecordExportReport {
    let mut report = RecordExportReport {
        written_record_indices,
        ..RecordExportReport::default()
    };
    for (index, outcome) in outcomes {
        let errors = match outcome {
            RecordOutcome::Written {
                errors,
                receipt,
                origin_failure,
            } => {
                report.written += 1;
                report.origin_receipts.extend(receipt);
                report.origin_failures.extend(origin_failure);
                if let (Some(written), Some(record)) =
                    (&mut report.written_record_indices, records.get(index))
                {
                    written.push(record.index());
                }
                errors
            }
            RecordOutcome::Kept { file, errors } => {
                report.skipped.push(file);
                errors
            }
            RecordOutcome::Cancelled => {
                report.cancelled += 1;
                0
            }
            RecordOutcome::Failed(error) => {
                let record = records.get(index);
                report.failures.push(RecordFailure {
                    guid: record.map_or_default(Record::guid),
                    record_path: record
                        .and_then(|record| record.record_path().ok())
                        .unwrap_or_default()
                        .to_owned(),
                    error,
                });
                0
            }
        };
        report.error_values += errors;
        report.records_with_errors += usize::from(errors > 0);
    }
    report
}
