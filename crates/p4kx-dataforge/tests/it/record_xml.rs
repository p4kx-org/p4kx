// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Writing the canonical Record tree as Record XML.

use std::error::Error;

use p4kx_dataforge::{DataForge, RecordXml};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

#[path = "record_xml_localization.rs"]
mod localization;

type TestResult = Result<(), Box<dyn Error>>;

const GUID: u128 = 0xcbee_34fd_6441_4000_b99f_924f_7f26_e19f;

fn xml_of(builder: &DataForgeBuilder) -> Result<String, Box<dyn Error>> {
    let bytes = builder.build();
    let forge = DataForge::open(&bytes)?;
    let tree = forge.records().next().ok_or("no Record")?.tree()?;
    let mut out = Vec::new();
    RecordXml::new().write(&tree, &mut out)?;
    Ok(String::from_utf8(out)?)
}

fn one_record(structure: TestStruct, record: TestRecord) -> DataForgeBuilder {
    DataForgeBuilder::new()
        .enumeration("Size", ["Small", "Large"])
        .structure(structure)
        .record(record)
}

fn record(type_name: &str) -> TestRecord {
    TestRecord::new(
        GUID,
        type_name,
        format!("{type_name}.Sample"),
        "libs/foundry/records/sample.xml",
    )
}

#[test]
fn scalars_are_attributes_of_the_root_named_after_the_type() -> TestResult {
    let builder = one_record(
        TestStruct::new("Scalars")
            .field("flag", TestType::Bool)
            .field("i8", TestType::Int8)
            .field("i16", TestType::Int16)
            .field("i32", TestType::Int32)
            .field("i64", TestType::Int64)
            .field("u8", TestType::UInt8)
            .field("u16", TestType::UInt16)
            .field("u32", TestType::UInt32)
            .field("u64", TestType::UInt64)
            .field("single", TestType::Single)
            .field("double", TestType::Double)
            .field("text", TestType::String)
            .field("label", TestType::Locale)
            .field("id", TestType::Guid)
            .field("size", TestType::Enum("Size".to_owned())),
        record("Scalars")
            .value("flag", TestValue::Bool(true))
            .value("i8", TestValue::Int8(-8))
            .value("i16", TestValue::Int16(-1600))
            .value("i32", TestValue::Int32(-320_000))
            .value("i64", TestValue::Int64(i64::MIN))
            .value("u8", TestValue::UInt8(200))
            .value("u16", TestValue::UInt16(65_000))
            .value("u32", TestValue::UInt32(4_000_000_000))
            .value("u64", TestValue::UInt64(u64::MAX))
            .value("single", TestValue::Single(1.5))
            .value("double", TestValue::Double(-0.25))
            .value("text", TestValue::text("Hello <World> & \"friends\""))
            .value("label", TestValue::text("@item_Name_Sample"))
            .value(
                "id",
                TestValue::Guid(0x50e8_a9f6_f956_4000_8ace_7c0c_a9a9_1f5d),
            )
            .value("size", TestValue::text("Large")),
    );

    let xml = xml_of(&builder)?;

    assert_eq!(
        xml,
        concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
            "<Scalars xmlns:p4kx=\"https://github.com/p4kx-org/p4kx/record-xml\" ",
            "guid=\"cbee34fd-6441-4000-b99f-924f7f26e19f\" ",
            "path=\"libs/foundry/records/sample\" p4kx:name=\"Scalars.Sample\" flag=\"true\" i8=\"-8\" i16=\"-1600\" ",
            "i32=\"-320000\" i64=\"-9223372036854775808\" u8=\"200\" u16=\"65000\" ",
            "u32=\"4000000000\" u64=\"18446744073709551615\" single=\"1.5\" double=\"-0.25\" ",
            "text=\"Hello &lt;World&gt; &amp; &quot;friends&quot;\" label=\"@item_Name_Sample\" ",
            "id=\"50e8a9f6-f956-4000-8ace-7c0ca9a91f5d\" size=\"Large\"/>\n",
        ),
        "one self-closing root with every scalar as an attribute"
    );
    Ok(())
}

#[test]
fn the_same_record_gives_byte_identical_xml() -> TestResult {
    let builder = one_record(
        TestStruct::new("Item")
            .field("mass", TestType::Single)
            .field("name", TestType::String)
            .field("broken", TestType::Locale),
        record("Item")
            .value("mass", TestValue::Single(0.1))
            .value("name", TestValue::text("Crate"))
            .value("broken", TestValue::Raw(vec![0xff; 4])),
    );

    let first = xml_of(&builder)?;
    let second = xml_of(&builder)?;

    assert_eq!(first, second, "two runs give the same bytes");
    assert!(!first.contains('\r'), "LF line endings only: {first:?}");
    Ok(())
}

#[test]
fn error_values_are_elements_with_the_field_and_the_reason() -> TestResult {
    let builder = one_record(
        TestStruct::new("Item")
            .field("count", TestType::Int32)
            .field("label", TestType::Locale)
            .field("size", TestType::Enum("Missing".to_owned())),
        record("Item")
            .value("count", TestValue::Int32(3))
            .value("label", TestValue::Raw(vec![0xff; 4])),
    );

    let xml = xml_of(&builder)?;

    assert_eq!(
        xml,
        concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
            "<Item xmlns:p4kx=\"https://github.com/p4kx-org/p4kx/record-xml\" ",
            "guid=\"cbee34fd-6441-4000-b99f-924f7f26e19f\" ",
            "path=\"libs/foundry/records/sample\" p4kx:name=\"Item.Sample\" count=\"3\">\n",
            "  <p4kx:error field=\"label\" ",
            "reason=\"offset 4294967295 in the text table is not a valid string\"/>\n",
            "  <p4kx:error field=\"size\" ",
            "reason=\"the field refers to enum 65535, which does not exist\"/>\n",
            "</Item>\n",
        ),
        "the namespace is declared and each error is a child element in field order"
    );
    Ok(())
}

#[test]
fn ambiguous_field_names_become_field_elements() -> TestResult {
    let builder = one_record(
        TestStruct::new("Item")
            .field("guid", TestType::Int8)
            .field("path", TestType::Int8)
            .field("type", TestType::Int8)
            .field("count", TestType::Int8)
            .field("count", TestType::Int8)
            .field("xmlns", TestType::Int8)
            .field("9lives", TestType::Int8)
            .field("a:b", TestType::Int8)
            .field("ok", TestType::Int8),
        record("Item")
            .value("guid", TestValue::Int8(1))
            .value("path", TestValue::Int8(2))
            .value("type", TestValue::Int8(3))
            .value("count", TestValue::Int8(4))
            .value("xmlns", TestValue::Int8(6))
            .value("9lives", TestValue::Int8(7))
            .value("a:b", TestValue::Int8(8))
            .value("ok", TestValue::Int8(9)),
    );

    let xml = xml_of(&builder)?;

    assert_eq!(
        xml,
        concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
            "<Item xmlns:p4kx=\"https://github.com/p4kx-org/p4kx/record-xml\" ",
            "guid=\"cbee34fd-6441-4000-b99f-924f7f26e19f\" ",
            "path=\"libs/foundry/records/sample\" p4kx:name=\"Item.Sample\" count=\"4\" ok=\"9\">\n",
            "  <p4kx:field name=\"guid\" value=\"1\"/>\n",
            "  <p4kx:field name=\"path\" value=\"2\"/>\n",
            "  <p4kx:field name=\"type\" value=\"3\"/>\n",
            "  <p4kx:field name=\"count\" value=\"0\"/>\n",
            "  <p4kx:field name=\"xmlns\" value=\"6\"/>\n",
            "  <p4kx:field name=\"9lives\" value=\"7\"/>\n",
            "  <p4kx:field name=\"a:b\" value=\"8\"/>\n",
            "</Item>\n",
        ),
        "reserved, repeated and invalid names keep their value in a field element"
    );
    Ok(())
}

#[test]
fn a_type_name_that_is_no_xml_name_becomes_a_record_element() -> TestResult {
    let builder = one_record(
        TestStruct::new("Bad Type").field("count", TestType::Int8),
        record("Bad Type").value("count", TestValue::Int8(1)),
    );

    let xml = xml_of(&builder)?;

    assert_eq!(
        xml,
        concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
            "<p4kx:record xmlns:p4kx=\"https://github.com/p4kx-org/p4kx/record-xml\" ",
            "type=\"Bad Type\" guid=\"cbee34fd-6441-4000-b99f-924f7f26e19f\" ",
            "path=\"libs/foundry/records/sample\" p4kx:name=\"Bad Type.Sample\" count=\"1\"/>\n",
        ),
        "the type moves into an attribute"
    );
    Ok(())
}

#[test]
fn text_is_escaped_and_characters_xml_cannot_hold_are_replaced() -> TestResult {
    let builder = one_record(
        TestStruct::new("Item")
            .field("text", TestType::String)
            .field("special", TestType::String)
            .field("umlaut", TestType::String),
        record("Item")
            .value("text", TestValue::text("a\tb\nc\rd'e"))
            .value("special", TestValue::text("x\u{1}y\u{fffe}z"))
            .value("umlaut", TestValue::text("Größe ✓")),
    );

    let xml = xml_of(&builder)?;

    assert!(
        xml.contains(
            " text=\"a&#9;b&#10;c&#13;d'e\" special=\"x\u{fffd}y\u{fffd}z\" umlaut=\"Größe ✓\"/>\n"
        ),
        "whitespace survives attribute normalisation, invalid characters become U+FFFD: {xml}"
    );
    Ok(())
}

#[test]
fn floats_have_one_text_form() -> TestResult {
    let builder = one_record(
        TestStruct::new("Item")
            .field("a", TestType::Single)
            .field("b", TestType::Single)
            .field("c", TestType::Single)
            .field("d", TestType::Double)
            .field("e", TestType::Double)
            .field("f", TestType::Double)
            .field("g", TestType::Single),
        record("Item")
            .value("a", TestValue::Single(0.1))
            .value("b", TestValue::Single(f32::INFINITY))
            .value("c", TestValue::Single(f32::NEG_INFINITY))
            .value("d", TestValue::Double(f64::NAN))
            .value("e", TestValue::Double(1e21))
            .value("f", TestValue::Double(-0.0))
            .value("g", TestValue::Single(3.0)),
    );

    let xml = xml_of(&builder)?;

    assert!(
        xml.contains(" a=\"0.1\" b=\"INF\" c=\"-INF\" d=\"NaN\" e=\"1e21\" f=\"-0\" g=\"3\"/>\n"),
        "shortest round-trip text, exponent for large and small magnitudes: {xml}"
    );
    Ok(())
}
