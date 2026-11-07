// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Filter, Guid};

use crate::Kind;
use crate::export_ledger::ExportLedger;
use crate::guid_text::CatalogGuid;

use super::Selection;

#[test]
fn only_actual_post_projection_row_exclusions_count_as_skipped_rows() {
    let mut ledger = ExportLedger::default();
    let filter = Filter::new("selected");
    let mut selection = Selection::new(Some(&filter), &mut ledger);
    let items = [
        ("selected/valid.xml", "Synthetic.Valid"),
        ("selected/synthetic_test.xml", "Synthetic.Test"),
        ("excluded/synthetic_test.xml", "Synthetic.Test"),
    ];
    let (rows, skipped) = selection.entities(
        Kind::GameplayProperties,
        &items,
        |item| *item,
        |_| CatalogGuid(Guid::default()),
        |guid| *guid,
    );
    assert_eq!(rows.len(), 1, "the actual selected normal row is emitted");
    assert_eq!(
        skipped, 1,
        "the outside-filter row is not an intentional selected-row skip"
    );
    assert_eq!(
        ledger.skipped_rows,
        [(Kind::GameplayProperties, 1)],
        "row units are captured at the actual row exclusion"
    );
    assert!(
        ledger.sources.is_empty(),
        "row selection does not fabricate canonical source outcomes"
    );
}
