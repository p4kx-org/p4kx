// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The flat value export: one row per leaf value of a Record.

use std::borrow::Cow;
use std::error::Error;

use p4kx_dataforge::{
    Annotations, DataForge, FlatFormat, FlatValues, Resolution, TextResolver, TreeBuilder,
};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

#[path = "record_flat_strict.rs"]
mod strict;

type TestResult = Result<(), Box<dyn Error>>;

const THING: u128 = 0xcbee_34fd_6441_4000_b99f_924f_7f26_e19f;
const MAKER: u128 = 0x50e8_a9f6_f956_4000_8ace_7c0c_a9a9_1f5d;
const OTHER: u128 = 0x0000_0001_0002_0003_0004_0000_0000_0005;
const HEAD: &str = "cbee34fd-6441-4000-b99f-924f7f26e19f,Thing.One,libs/things/one,";
const MAKER_TEXT: &str = "50e8a9f6-f956-4000-8ace-7c0ca9a91f5d";

struct Texts;

impl TextResolver for Texts {
    fn resolve<'a>(&'a self, key: &str) -> Resolution<'a> {
        match key {
            "@item_Name" => Resolution::Text(Cow::Borrowed("=Crate, \"big\"")),
            "@LOC_EMPTY" => Resolution::Keep,
            _ => Resolution::Missing,
        }
    }
}

fn wheel(size: i32, marks: &[u8]) -> TestValue {
    TestValue::Struct(
        TestInstance::new("Wheel")
            .value("size", TestValue::Int32(size))
            .value(
                "marks",
                TestValue::Array(marks.iter().map(|mark| TestValue::UInt8(*mark)).collect()),
            ),
    )
}

fn builder(name: &str) -> DataForgeBuilder {
    DataForgeBuilder::new()
        .enumeration("Size", ["Small", "Large"])
        .structure(
            TestStruct::new("Wheel")
                .field("size", TestType::Int32)
                .array("marks", TestType::UInt8),
        )
        .structure(TestStruct::new("Maker"))
        .structure(
            TestStruct::new("Thing")
                .field("flag", TestType::Bool)
                .field("small", TestType::Int8)
                .field("count", TestType::Int32)
                .field("wide", TestType::Int64)
                .field("tiny", TestType::UInt8)
                .field("huge", TestType::UInt64)
                .field("single", TestType::Single)
                .field("double", TestType::Double)
                .field("name", TestType::String)
                .field("label", TestType::Locale)
                .field("size", TestType::Enum("Size".to_owned()))
                .field("id", TestType::Guid)
                .field("maker", TestType::Reference)
                .field("lost", TestType::Reference)
                .field("empty", TestType::String)
                .field("blank", TestType::Locale)
                .field("spare", TestType::StrongPointer("Wheel".to_owned()))
                .field("none", TestType::StrongPointer("Wheel".to_owned()))
                .field("front", TestType::WeakPointer("Wheel".to_owned()))
                .array("wheels", TestType::Class("Wheel".to_owned()))
                .array("tags", TestType::String)
                .array("nothing", TestType::String),
        )
        .instance(
            "spare",
            TestInstance::new("Wheel").value("size", TestValue::Int32(15)),
        )
        .record(thing(name))
        .record(TestRecord::new(
            MAKER,
            "Maker",
            "Maker.Aegis",
            "libs/makers/aegis.xml",
        ))
}

fn thing(name: &str) -> TestRecord {
    TestRecord::new(THING, "Thing", "Thing.One", "libs/things/one.xml")
        .value("flag", TestValue::Bool(true))
        .value("small", TestValue::Int8(-5))
        .value("count", TestValue::Int32(0))
        .value("wide", TestValue::Int64(i64::MIN))
        .value("tiny", TestValue::UInt8(7))
        .value("huge", TestValue::UInt64(u64::MAX))
        .value("single", TestValue::Single(0.1))
        .value("double", TestValue::Double(2.5))
        .value("name", TestValue::text(name))
        .value("label", TestValue::text("@item_Name"))
        .value("size", TestValue::text("Large"))
        .value("id", TestValue::Guid(OTHER))
        .value("maker", TestValue::Reference(MAKER))
        .value("lost", TestValue::Null)
        .value("empty", TestValue::text(""))
        .value("blank", TestValue::text(""))
        .value("spare", TestValue::pointer("spare"))
        .value("none", TestValue::Null)
        .value("front", TestValue::pointer("spare"))
        .value(
            "wheels",
            TestValue::Array(vec![wheel(16, &[]), wheel(17, &[1, 2])]),
        )
        .value(
            "tags",
            TestValue::Array(vec![TestValue::text("a"), TestValue::text("")]),
        )
        .value("nothing", TestValue::Array(Vec::new()))
}

fn flat(
    name: &str,
    format: FlatFormat,
    texts: Option<&dyn TextResolver>,
) -> Result<String, Box<dyn Error>> {
    let bytes = builder(name).build();
    let forge = DataForge::open(&bytes)?;
    let record = forge.records().next().ok_or("no Record")?;
    let tree = TreeBuilder::new().build(&record)?;
    let mut writer = FlatValues::new(format);
    let mut out = Vec::new();
    writer.write_header(texts.is_some(), &mut out)?;
    writer.write_annotated(&tree, Annotations::from(texts), &mut out)?;
    Ok(String::from_utf8(out)?)
}

fn rows(text: &str) -> Vec<&str> {
    text.split("\r\n").filter(|row| !row.is_empty()).collect()
}

#[test]
fn csv_starts_with_a_byte_order_mark_and_the_header_and_ends_rows_with_crlf() -> TestResult {
    let text = flat("n", FlatFormat::Csv, None)?;

    assert!(text.starts_with(
        "\u{feff}record_guid,record_name,record_path,field_path,array_indices,kind,value,target_path,record_tag\r\n"
    ));
    assert!(text.ends_with("\r\n"));
    assert!(!text.replace("\r\n", "").contains('\n'));
    Ok(())
}

#[test]
fn every_kind_has_one_row_with_its_value() -> TestResult {
    let text = flat("n", FlatFormat::CsvRaw, None)?;
    let rows = rows(&text);

    for expected in [
        "flag,,bool,true,,",
        "small,,int,-5,,",
        "count,,int,0,,",
        "wide,,int,-9223372036854775808,,",
        "tiny,,uint,7,,",
        "huge,,uint,18446744073709551615,,",
        "single,,single,0.1,,",
        "double,,double,2.5,,",
        "name,,string,n,,",
        "label,,locale,@item_Name,,",
        "size,,enum,Large,,",
        "id,,guid,00000001-0002-0003-0004-000000000005,,",
    ] {
        let row = format!("{HEAD}{expected}");
        assert!(rows.contains(&row.as_str()), "{row} in {rows:#?}");
    }
    Ok(())
}

#[test]
fn raw_csv_has_no_byte_order_mark() -> TestResult {
    let text = flat("n", FlatFormat::CsvRaw, None)?;

    assert!(text.starts_with("record_guid,"));
    Ok(())
}

#[test]
fn references_carry_the_target_guid_and_record_path() -> TestResult {
    let text = flat("n", FlatFormat::CsvRaw, None)?;
    let rows = rows(&text);

    assert!(
        rows.contains(&format!("{HEAD}maker,,reference,{MAKER_TEXT},libs/makers/aegis,").as_str())
    );
    assert!(
        rows.contains(
            &format!(
                "{HEAD}front,,reference,cbee34fd-6441-4000-b99f-924f7f26e19f,libs/things/one,"
            )
            .as_str()
        ),
        "{rows:#?}"
    );
    Ok(())
}

#[test]
fn empty_values_get_no_row() -> TestResult {
    let text = flat("n", FlatFormat::CsvRaw, None)?;

    for field in ["lost", "empty", "blank", "none", "nothing"] {
        assert!(
            !text.contains(&format!(",{field},")),
            "{field} has no row: {text}"
        );
    }
    assert_eq!(
        rows(&text)
            .iter()
            .filter(|row| row.contains(",tags,"))
            .count(),
        1,
        "only the non-empty tag"
    );
    Ok(())
}

#[test]
fn array_positions_leave_the_field_path_and_fill_the_indices() -> TestResult {
    let text = flat("n", FlatFormat::CsvRaw, None)?;
    let rows = rows(&text);

    for expected in [
        "wheels.size,0,int,16,,",
        "wheels.size,1,int,17,,",
        "wheels.marks,1.0,uint,1,,",
        "wheels.marks,1.1,uint,2,,",
        "tags,0,string,a,,",
        "spare.size,,int,15,,",
    ] {
        let row = format!("{HEAD}{expected}");
        assert!(rows.contains(&row.as_str()), "{row} in {rows:#?}");
    }
    Ok(())
}

#[test]
fn rows_follow_the_field_order() -> TestResult {
    let text = flat("n", FlatFormat::CsvRaw, None)?;
    let fields: Vec<&str> = rows(&text)
        .iter()
        .skip(1)
        .filter_map(|row| row.split(',').nth(3))
        .collect();

    assert_eq!(fields.first(), Some(&"flag"));
    assert_eq!(fields.last(), Some(&"tags"));
    Ok(())
}

#[test]
fn csv_quotes_cells_with_commas_quotes_and_line_breaks() -> TestResult {
    let text = flat("a,\"b\"\nc", FlatFormat::CsvRaw, None)?;

    assert!(
        text.contains(",name,,string,\"a,\"\"b\"\"\nc\",,"),
        "{text}"
    );
    Ok(())
}

#[test]
fn csv_neutralises_formulas_in_text_kinds_only() -> TestResult {
    let text = flat("=1+1", FlatFormat::Csv, None)?;

    assert!(text.contains(",name,,string,'=1+1,,"), "{text}");
    assert!(
        text.contains(",small,,int,-5,,"),
        "negative numbers stay numbers"
    );
    for lead in ["+x", "-x", "@x", "\tx"] {
        let text = flat(lead, FlatFormat::Csv, None)?;
        assert!(text.contains(&format!(",string,'{lead},,")), "{lead}");
    }
    let text = flat("\rx", FlatFormat::Csv, None)?;
    assert!(text.contains(",string,\"'\rx\",,"));
    Ok(())
}

#[test]
fn raw_csv_changes_no_value() -> TestResult {
    let text = flat("=1+1", FlatFormat::CsvRaw, None)?;

    assert!(text.contains(",name,,string,=1+1,,"), "{text}");
    Ok(())
}

#[test]
fn texts_add_a_column_filled_for_resolved_locale_values() -> TestResult {
    let text = flat("n", FlatFormat::Csv, Some(&Texts))?;
    let rows = rows(&text);

    assert!(
        rows.first()
            .is_some_and(|header| header.ends_with(",target_path,text,record_tag"))
    );
    assert!(
        rows.contains(
            &format!("{HEAD}label,,locale,'@item_Name,,\"'=Crate, \"\"big\"\"\",").as_str()
        ),
        "{rows:#?}"
    );
    assert!(rows.contains(&format!("{HEAD}flag,,bool,true,,,").as_str()));
    Ok(())
}

#[test]
fn a_missing_text_leaves_the_cell_empty() -> TestResult {
    struct Gone;
    impl TextResolver for Gone {
        fn resolve<'a>(&'a self, _key: &str) -> Resolution<'a> {
            Resolution::Missing
        }
    }

    let text = flat("n", FlatFormat::CsvRaw, Some(&Gone))?;

    assert!(text.contains(&format!("{HEAD}label,,locale,@item_Name,,,\r\n")));
    Ok(())
}

#[test]
fn ndjson_has_one_object_per_value_without_byte_order_mark() -> TestResult {
    let text = flat("=n", FlatFormat::Ndjson, None)?;

    assert!(text.starts_with('{'));
    let values: Vec<serde_json::Value> = text
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    let find = |field: &str, indices: &str| {
        values
            .iter()
            .find(|value| value["field_path"] == field && value["array_indices"] == indices)
    };

    let huge = find("huge", "").ok_or("huge")?;
    assert_eq!(huge["kind"], "uint");
    assert_eq!(huge["value"].to_string(), "18446744073709551615");
    assert_eq!(huge["record_guid"], "cbee34fd-6441-4000-b99f-924f7f26e19f");
    assert_eq!(huge["record_name"], "Thing.One");
    assert_eq!(huge["record_path"], "libs/things/one");
    assert!(huge.get("target_path").is_none());
    assert!(huge.get("record_tag").is_none());
    assert_eq!(
        find("single", "").ok_or("single")?["value"].to_string(),
        "0.1"
    );
    assert_eq!(find("name", "").ok_or("name")?["value"], "=n");
    assert_eq!(find("flag", "").ok_or("flag")?["value"], true);
    let maker = find("maker", "").ok_or("maker")?;
    assert_eq!(maker["kind"], "reference");
    assert_eq!(maker["value"], MAKER_TEXT);
    assert_eq!(maker["target_path"], "libs/makers/aegis");
    assert_eq!(find("wheels.marks", "1.1").ok_or("marks")?["value"], 2);
    Ok(())
}

#[test]
fn ndjson_adds_the_text_of_resolved_locale_values() -> TestResult {
    let text = flat("n", FlatFormat::Ndjson, Some(&Texts))?;

    let label = text
        .lines()
        .map(serde_json::from_str::<serde_json::Value>)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .find(|value| value["field_path"] == "label")
        .ok_or("label")?;
    assert_eq!(label["text"], "=Crate, \"big\"");
    Ok(())
}

#[test]
fn the_same_tree_always_gives_the_same_bytes() -> TestResult {
    assert_eq!(
        flat("n", FlatFormat::Csv, None)?,
        flat("n", FlatFormat::Csv, None)?
    );
    Ok(())
}
