// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_dataforge::DataForge;
use p4kx_dataforge::query::{Query, QueryUnavailable};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

use crate::{SourceInput, SourceLocation, SourceUseRole};

use super::FieldStatisticsReport;

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn original_empty_string_and_locale_each_use_their_authored_nul() -> TestResult {
    let bytes = fixture();
    let forge = DataForge::open(&bytes)?;
    for field in ["title", "localeKey"] {
        let query = Query::parse(&format!(r#"from Owner at "libs/empty" where {field} = """#))?;
        let report = FieldStatisticsReport::query(&forge, SourceInput::Unknown, &query)?;
        assert_eq!(
            report.reduction.statistics().records(),
            1,
            "the original {field} is an actual empty string"
        );
        assert!(
            sourced_nul(&report, &bytes)?,
            "the original text terminator, not an empty payload proxy, supplies the comparison"
        );
    }
    Ok(())
}

#[test]
fn unreadable_text_offset_does_not_supply_an_empty_input() -> TestResult {
    let bytes = fixture();
    let forge = DataForge::open(&bytes)?;
    let query = Query::parse(r#"from Owner at "libs/bad" where title = "" or fallback = true"#)?;
    let report = FieldStatisticsReport::query(&forge, SourceInput::Unknown, &query)?;
    assert_eq!(
        report.reduction.statistics().records(),
        1,
        "the decisive second clause admits the Record"
    );
    assert_eq!(
        report.selection_attempts.len(),
        1,
        "the actual failed text offset is retained"
    );
    assert!(
        report.unavailable_traversals.iter().any(|event| matches!(
            event,
            QueryUnavailable::Text {
                offset: u32::MAX,
                ..
            }
        )),
        "the invalid offset remains explicit without a fabricated original text span"
    );
    assert!(
        !sourced_nul(&report, &bytes)?,
        "an invalid text offset has no original empty text contribution"
    );
    Ok(())
}

fn sourced_nul(report: &FieldStatisticsReport<'_>, bytes: &[u8]) -> Result<bool, Box<dyn Error>> {
    for site in report.inventory.iter_uses() {
        let SourceLocation::Subresource {
            namespace,
            indices,
            field,
        } = site.location()
        else {
            continue;
        };
        if namespace != "dataforgeDecodedBytes"
            || field != "text"
            || site.role() != SourceUseRole::Selector
            || !report
                .selection_origin
                .source_uses_refs()
                .contains(site.id())
        {
            continue;
        }
        let [start, end] = indices.as_slice() else {
            return Err("the original text locator addresses one byte span".into());
        };
        let span = usize::try_from(*start)?..usize::try_from(*end)?;
        if matches!(bytes.get(span), Some([0])) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn fixture() -> Vec<u8> {
    DataForgeBuilder::new()
        .structure(
            TestStruct::new("Owner")
                .field("title", TestType::String)
                .field("localeKey", TestType::Locale)
                .field("fallback", TestType::Bool),
        )
        .record(
            TestRecord::new(1, "Owner", "Empty", "libs/empty.xml")
                .value("title", TestValue::text(""))
                .value("localeKey", TestValue::text("")),
        )
        .record(
            TestRecord::new(2, "Owner", "Broken", "libs/bad.xml")
                .value("title", TestValue::Raw(u32::MAX.to_le_bytes().to_vec()))
                .value("localeKey", TestValue::text("example_localization_key"))
                .value("fallback", TestValue::Bool(true)),
        )
        .build()
}
