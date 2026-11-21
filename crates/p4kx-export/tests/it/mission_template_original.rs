// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DataForge;
use p4kx_testkit::DataForgeBuilder;

use crate::health_origin_fixture::TestResult;

/// Verifies the actual controlled Template owner before any catalog producer is called.
pub(crate) fn owner_is_first(builder: &DataForgeBuilder) -> TestResult {
    let bytes = builder.build();
    let forge = DataForge::open(&bytes)?;
    let wanted = crate::catalog_crafting_support::text(crate::scoped_fixture::TEMPLATE);
    let records: Vec<_> = forge
        .records()
        .filter(|record| record.guid().to_string() == wanted)
        .collect();
    assert_eq!(
        records.len(),
        1,
        "the actual source contains one physical Template with this controlled GUID"
    );
    let record = records.first().ok_or("the original controlled Template")?;
    assert_eq!(
        record.type_name()?,
        "ContractTemplate",
        "the genuine physical owner has the declared source family"
    );
    assert_eq!(
        record.index(),
        0,
        "mission_values_fixture stores the Template before its linked Generator and target Definition"
    );
    let tree = record.tree()?;
    assert!(
        tree.is_from(record),
        "the independent canonical source read belongs to that exact physical Record"
    );
    assert_eq!(
        tree.guid(),
        record.guid(),
        "the actual canonical source keeps the original GUID"
    );
    Ok(())
}
