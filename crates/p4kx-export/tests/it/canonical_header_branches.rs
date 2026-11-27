// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_export::CatalogPublicationMode;
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct};
use serde_json::json;

use crate::canonical_header_fixture as fixture;
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};

#[test]
fn declared_empty_unmatched_and_empty_suffix_names_keep_the_actual_prefix_algorithm() -> TestResult
{
    for (name, expected) in [
        ("", ""),
        ("Other.Name", "Other.Name"),
        ("ResourceTypeGroup.", "ResourceTypeGroup."),
        ("ResourceTypeGroup.Match", "Match"),
        (" ResourceTypeGroup.Space", " ResourceTypeGroup.Space"),
    ] {
        check_name(name, expected)?;
    }
    Ok(())
}

fn check_name(name: &str, expected: &str) -> TestResult {
    let source = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("ResourceTypeGroup"))
        .record(TestRecord::new(
            fixture::GROUP,
            "ResourceTypeGroup",
            name,
            "libs/foundry/records/resourcetypedatabase/Name.XML",
        ));
    let (_, catalog) = fixture::build(
        &source,
        DcbOrigin::Plain,
        CatalogPublicationMode::DiagnosticPartial,
        None,
    )?;
    let row = fixture::row(&catalog, "definitions/resourceGroups", fixture::GROUP)?;
    assert_eq!(
        common::at(row, "/recordName/state")?,
        &json!("present"),
        "a stored empty name is not a no-string sentinel"
    );
    assert_eq!(
        common::at(row, "/recordName/value")?,
        &json!(name),
        "the exact original declared name survives"
    );
    assert_eq!(
        common::at(row, "/className/value")?,
        &json!(expected),
        "only the established nonempty type-prefix removal applies"
    );
    assert_eq!(
        fixture::header_uses(&catalog, common::at(row, "/className")?)?,
        BTreeSet::from([
            (0, "recordName".to_owned(), "conversion".to_owned()),
            (0, "sourceType".to_owned(), "selector".to_owned())
        ]),
        "the actual attempted prefix operation uses the stored name and original type"
    );
    Ok(())
}
