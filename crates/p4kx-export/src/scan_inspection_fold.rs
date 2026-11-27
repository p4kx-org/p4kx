// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

use crate::export_ledger::ExportLedger;

use super::{FieldInspection, InspectionKey};

enum Position {
    First(usize),
    Preserve,
}

/// Releases repeated immutable scan proofs without advancing final accounting errors.
#[derive(Default)]
pub(crate) struct ScanInspectionFold {
    positions: BTreeMap<InspectionKey, Position>,
}

impl ScanInspectionFold {
    pub(crate) fn start(ledger: &mut ExportLedger) -> Self {
        let mut fold = Self::default();
        let inspections = std::mem::take(&mut ledger.field_inspections);
        fold.append(&mut ledger.field_inspections, inspections);
        fold
    }

    pub(crate) fn merge(&mut self, ledger: &mut ExportLedger, mut read: ExportLedger) {
        self.append(
            &mut ledger.field_inspections,
            std::mem::take(&mut read.field_inspections),
        );
        ledger.merge(read);
    }

    fn append(&mut self, retained: &mut Vec<FieldInspection>, incoming: Vec<FieldInspection>) {
        for mut inspection in incoming {
            if let Ok(key) = inspection.key() {
                match self.positions.entry(key) {
                    Entry::Vacant(entry) => {
                        entry.insert(Position::First(retained.len()));
                    }
                    Entry::Occupied(mut entry) => {
                        if let Position::First(position) = entry.get()
                            && let Some(previous) = retained.get(*position)
                            && previous.absorbs_scan_duplicate(&mut inspection, key)
                        {
                            continue;
                        }
                        entry.insert(Position::Preserve);
                    }
                }
            }
            retained.push(inspection);
        }
    }
}

#[cfg(test)]
#[path = "scan_inspection_fold_tests.rs"]
mod tests;
