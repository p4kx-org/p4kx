// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_dataforge::DataForge;
use p4kx_dataforge::query::Query;
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

use crate::{SourceInput, SourceLocation, SourceUseRole};

use super::FieldStatisticsReport;
use super::instance_tests::selector;

type TestResult = Result<(), Box<dyn Error>>;

const CORRUPTED_TEXT: &str = "OriginallyReadableButDamaged";

#[test]
fn enum_keeps_each_consulted_declaration_and_the_original_count_bound() -> TestResult {
    let bytes = fixture();
    let forge = DataForge::open(&bytes)?;
    let query = Query::parse(r#"at "libs/enums/valid/*" where state = "Open""#)?;
    let report = FieldStatisticsReport::query(&forge, SourceInput::Unknown, &query)?;
    assert_eq!(
        report.reduction.statistics().records(),
        2,
        "the two original enum scopes admit"
    );
    for index in [0, 2] {
        assert!(
            selector(&report, "dataforgePropertyDefinition", &[index], "target"),
            "a repeated supported enum key still consults the actual original target declaration"
        );
    }
    assert!(
        selector(&report, "dataforgeHeader", &[], "enumCount")
            && selector(&report, "dataforgeHeader", &[], "textLength"),
        "the actual enum count and text table length both constrain the comparison"
    );
    assert!(
        report.inventory.iter_uses().all(|site| {
            !matches!(site.location(), SourceLocation::Subresource { namespace, .. }
                if namespace == "dataforgeEnumDefinition")
        }),
        "the decoder does not read the enum options to compare this authored value"
    );
    Ok(())
}

#[test]
fn invalid_enum_target_checks_count_before_attempting_any_text_read() -> TestResult {
    let bytes = fixture();
    let forge = DataForge::open(&bytes)?;
    let query = Query::parse(r#"from BrokenEnum where state = "Open" or fallback = true"#)?;
    let report = FieldStatisticsReport::query(&forge, SourceInput::Unknown, &query)?;
    assert_eq!(
        report.reduction.statistics().records(),
        1,
        "the readable branch preserves admission"
    );
    assert!(
        selector(&report, "dataforgePropertyDefinition", &[3], "target")
            && selector(&report, "dataforgeHeader", &[], "enumCount"),
        "the bad target word and original enum extent actually decide the failed read"
    );
    assert!(
        !selector(&report, "dataforgeHeader", &[], "textLength"),
        "the text table remains untouched after rejection of the enum target"
    );
    assert_eq!(
        report.selection_attempts.len(),
        1,
        "one original scalar offset is attempted"
    );
    Ok(())
}

#[test]
fn nonempty_string_and_locale_use_the_authored_termination() -> TestResult {
    let bytes = fixture();
    let forge = DataForge::open(&bytes)?;
    for column in ["title", "localeKey"] {
        let query = Query::parse(&format!(
            r#"from TextOwner at "libs/text/good" where {column} = "Ready""#
        ))?;
        let report = FieldStatisticsReport::query(&forge, SourceInput::Unknown, &query)?;
        assert_eq!(
            report.reduction.statistics().records(),
            1,
            "the original {column} value is read"
        );
        assert!(
            selector(&report, "dataforgeHeader", &[], "textLength"),
            "the actual original text-table extent is a selector"
        );
        let range = span_of(&report, "textTerminator", SourceUseRole::Selector)?;
        assert_eq!(
            bytes.get(range),
            Some([0_u8].as_slice()),
            "the original NUL terminator is checked"
        );
    }
    Ok(())
}

#[test]
fn invalid_utf8_keeps_attempted_original_text_through_its_terminator() -> TestResult {
    let mut bytes = fixture();
    let marker = CORRUPTED_TEXT.as_bytes();
    let mut positions = bytes
        .windows(marker.len())
        .enumerate()
        .filter_map(|(start, window)| (window == marker).then_some(start));
    let start = positions
        .next()
        .ok_or("the original synthetic text start exists")?;
    assert_eq!(
        positions.next(),
        None,
        "only the intended original text is modified"
    );
    *bytes
        .get_mut(start)
        .ok_or("the bounded original text byte exists")? = u8::MAX;
    let forge = DataForge::open(&bytes)?;
    let query =
        Query::parse(r#"from TextOwner at "libs/text/bad" where title = "" or fallback = true"#)?;
    let report = FieldStatisticsReport::query(&forge, SourceInput::Unknown, &query)?;
    assert_eq!(
        report.reduction.statistics().records(),
        1,
        "the fallback determines this Record"
    );
    let attempt = span_of(&report, "text", SourceUseRole::Attempted)?;
    assert_eq!(
        attempt,
        start..start + marker.len() + 1,
        "the failed decode retains its actual read extent through NUL"
    );
    assert!(
        report.unavailable_traversals.is_empty(),
        "readable original bytes exist even though UTF-8 decode fails"
    );
    Ok(())
}

fn span_of(
    report: &FieldStatisticsReport<'_>,
    field: &str,
    role: SourceUseRole,
) -> Result<std::ops::Range<usize>, Box<dyn Error>> {
    let uses = report.inventory.iter_uses();
    for site in uses {
        let refs = if role == SourceUseRole::Attempted {
            report.selection_attempts.as_slice()
        } else {
            report.selection_origin.source_uses_refs()
        };
        if site.role() != role || !refs.contains(site.id()) {
            continue;
        }
        if let SourceLocation::Subresource {
            namespace,
            indices,
            field: at,
        } = site.location()
            && namespace == "dataforgeDecodedBytes"
            && at == field
            && let [start, end] = indices.as_slice()
        {
            return Ok(usize::try_from(*start)?..usize::try_from(*end)?);
        }
    }
    Err("the requested original physical use must resolve".into())
}

fn fixture() -> Vec<u8> {
    DataForgeBuilder::new()
        .enumeration("State", ["Open", "Closed"])
        .structure(
            TestStruct::new("FirstEnum")
                .field("state", TestType::Enum("State".into()))
                .field("fallback", TestType::Bool),
        )
        .structure(TestStruct::new("SecondEnum").field("state", TestType::Enum("State".into())))
        .structure(
            TestStruct::new("BrokenEnum")
                .field("state", TestType::Enum("Unknown".into()))
                .field("fallback", TestType::Bool),
        )
        .structure(
            TestStruct::new("TextOwner")
                .field("title", TestType::String)
                .field("localeKey", TestType::Locale)
                .field("fallback", TestType::Bool),
        )
        .record(
            TestRecord::new(1, "FirstEnum", "First", "libs/enums/valid/first.xml")
                .value("state", TestValue::text("Open")),
        )
        .record(
            TestRecord::new(2, "SecondEnum", "Second", "libs/enums/valid/second.xml")
                .value("state", TestValue::text("Open")),
        )
        .record(
            TestRecord::new(3, "BrokenEnum", "BrokenEnum", "libs/enums/bad.xml")
                .value("state", TestValue::text("Open"))
                .value("fallback", TestValue::Bool(true)),
        )
        .record(
            TestRecord::new(4, "TextOwner", "Text", "libs/text/good.xml")
                .value("title", TestValue::text("Ready"))
                .value("localeKey", TestValue::text("Ready")),
        )
        .record(
            TestRecord::new(5, "TextOwner", "BrokenText", "libs/text/bad.xml")
                .value("title", TestValue::text(CORRUPTED_TEXT))
                .value("fallback", TestValue::Bool(true)),
        )
        .build()
}
