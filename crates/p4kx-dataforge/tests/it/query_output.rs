// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Writing Query results as CSV and JSON: the header, quoting, escaping, the typed values and the
//! formulas a spreadsheet would run.

use std::error::Error;
use std::io::ErrorKind;

use p4kx_dataforge::query::{ResultFormat, ResultWriter};
use p4kx_dataforge::{Guid, Value, ValueError};

type TestResult = Result<(), Box<dyn Error>>;

fn columns(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

fn written(
    format: ResultFormat,
    names: &[&str],
    rows: &[(&str, Vec<Option<Value<'_>>>)],
) -> Result<String, Box<dyn Error>> {
    let mut writer = ResultWriter::new(Vec::new(), format, &columns(names))?;
    for (record_path, cells) in rows {
        writer.write_row(record_path, cells)?;
    }
    Ok(String::from_utf8(writer.finish()?)?)
}

#[test]
fn csv_starts_with_a_header_row_and_ends_every_row_with_crlf() -> TestResult {
    let csv = written(
        ResultFormat::CsvRaw,
        &["mass", "size"],
        &[(
            "libs/a",
            vec![Some(Value::Single(2.5)), Some(Value::Enum("Small"))],
        )],
    )?;

    assert_eq!(
        csv, "record path,mass,size\r\nlibs/a,2.5,Small\r\n",
        "header and row"
    );
    Ok(())
}

#[test]
fn csv_quotes_fields_with_separators_quotes_and_line_breaks() -> TestResult {
    let csv = written(
        ResultFormat::CsvRaw,
        &["a", "b", "c", "d", "e"],
        &[(
            "libs/x,y",
            vec![
                Some(Value::String("say \"hi\"")),
                Some(Value::String("two\nlines")),
                Some(Value::String("cr\rhere")),
                Some(Value::String(" plain spaces ")),
                Some(Value::String("")),
            ],
        )],
    )?;

    assert_eq!(
        csv,
        "record path,a,b,c,d,e\r\n\"libs/x,y\",\"say \"\"hi\"\"\",\"two\nlines\",\"cr\rhere\", plain spaces ,\r\n",
        "quoted where needed"
    );
    Ok(())
}

#[test]
fn csv_quotes_a_header_that_needs_it() -> TestResult {
    let csv = written(ResultFormat::CsvRaw, &["`a,b`", "`say \"x\"`"], &[])?;

    assert_eq!(
        csv, "record path,\"`a,b`\",\"`say \"\"x\"\"`\"\r\n",
        "quoted header"
    );
    Ok(())
}

#[test]
fn csv_writes_values_as_record_xml_does_and_a_missing_value_as_an_empty_field() -> TestResult {
    let guid: Guid = "cbee34fd-6441-4000-b99f-924f7f26e19f".parse()?;
    let csv = written(
        ResultFormat::CsvRaw,
        &["a", "b", "c", "d", "e", "f", "g", "h", "i"],
        &[(
            "libs/r",
            vec![
                Some(Value::Bool(true)),
                Some(Value::Int64(i64::MIN)),
                Some(Value::UInt64(u64::MAX)),
                Some(Value::Double(1e21)),
                Some(Value::Single(f32::NEG_INFINITY)),
                Some(Value::Locale("@item_Name")),
                Some(Value::Guid(guid)),
                Some(Value::Error(ValueError::InvalidBool { byte: 7 })),
                None,
            ],
        )],
    )?;

    assert_eq!(
        csv,
        "record path,a,b,c,d,e,f,g,h,i\r\nlibs/r,true,-9223372036854775808,18446744073709551615,1e21,-INF,@item_Name,cbee34fd-6441-4000-b99f-924f7f26e19f,error: the bool byte 7 is neither 0 nor 1,\r\n",
        "Record XML texts"
    );
    Ok(())
}

#[test]
fn csv_starts_with_a_byte_order_mark_and_the_raw_csv_does_not() -> TestResult {
    let cells = || vec![Some(Value::Int32(1))];

    let csv = written(ResultFormat::Csv, &["a"], &[("libs/r", cells())])?;
    let raw = written(ResultFormat::CsvRaw, &["a"], &[("libs/r", cells())])?;

    assert_eq!(csv, "\u{feff}record path,a\r\nlibs/r,1\r\n", "default CSV");
    assert_eq!(raw, "record path,a\r\nlibs/r,1\r\n", "raw CSV");
    Ok(())
}

#[test]
fn csv_neutralises_text_that_a_spreadsheet_would_run_as_a_formula() -> TestResult {
    let row = vec![
        Some(Value::String("=1+1")),
        Some(Value::String("+a")),
        Some(Value::String("-a")),
        Some(Value::String("@SUM(A1)")),
        Some(Value::String("\tx")),
        Some(Value::String("\rx")),
        Some(Value::Locale("@item_Name")),
        Some(Value::Enum("-Small")),
        Some(Value::String("=a,\"b\"")),
        Some(Value::String("a=b")),
    ];
    let names = ["a", "b", "c", "d", "e", "f", "g", "h", "i", "j"];

    let csv = written(ResultFormat::Csv, &names, &[("libs/r", row.clone())])?;
    let raw = written(ResultFormat::CsvRaw, &names, &[("libs/r", row)])?;

    assert_eq!(
        csv,
        "\u{feff}record path,a,b,c,d,e,f,g,h,i,j\r\nlibs/r,'=1+1,'+a,'-a,'@SUM(A1),'\tx,\"'\rx\",'@item_Name,'-Small,\"'=a,\"\"b\"\"\",a=b\r\n",
        "text cells start with an apostrophe"
    );
    assert_eq!(
        raw,
        "record path,a,b,c,d,e,f,g,h,i,j\r\nlibs/r,=1+1,+a,-a,@SUM(A1),\tx,\"\rx\",@item_Name,-Small,\"=a,\"\"b\"\"\",a=b\r\n",
        "raw cells are exact"
    );
    Ok(())
}

#[test]
fn csv_never_changes_a_number() -> TestResult {
    let row = vec![
        Some(Value::Int32(-5)),
        Some(Value::Double(-1.5)),
        Some(Value::Single(f32::NEG_INFINITY)),
        Some(Value::Int64(i64::MIN)),
    ];

    let csv = written(ResultFormat::Csv, &["a", "b", "c", "d"], &[("libs/r", row)])?;

    assert_eq!(
        csv, "\u{feff}record path,a,b,c,d\r\nlibs/r,-5,-1.5,-INF,-9223372036854775808\r\n",
        "numbers keep their sign"
    );
    Ok(())
}

#[test]
fn json_is_one_line_holding_an_array_of_objects_keyed_by_column() -> TestResult {
    let json = written(
        ResultFormat::Json,
        &["mass", "size"],
        &[
            (
                "libs/a",
                vec![Some(Value::Single(2.5)), Some(Value::Enum("Small"))],
            ),
            ("libs/b", vec![Some(Value::Single(-0.0)), None]),
        ],
    )?;

    assert_eq!(
        json,
        "[{\"record path\":\"libs/a\",\"mass\":2.5,\"size\":\"Small\"},{\"record path\":\"libs/b\",\"mass\":-0,\"size\":null}]\n",
        "one line of objects"
    );
    Ok(())
}

#[test]
fn json_of_an_empty_result_is_an_empty_array() -> TestResult {
    assert_eq!(
        written(ResultFormat::Json, &["mass"], &[])?,
        "[]\n",
        "empty array"
    );
    Ok(())
}

#[test]
fn json_types_every_value_kind() -> TestResult {
    let guid: Guid = "cbee34fd-6441-4000-b99f-924f7f26e19f".parse()?;
    let json = written(
        ResultFormat::Json,
        &["a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k"],
        &[(
            "libs/r",
            vec![
                Some(Value::Bool(false)),
                Some(Value::Int8(-8)),
                Some(Value::UInt64(u64::MAX)),
                Some(Value::Double(1.5e-7)),
                Some(Value::Single(f32::NAN)),
                Some(Value::Double(f64::INFINITY)),
                Some(Value::String("text")),
                Some(Value::Locale("@item_Name")),
                Some(Value::Guid(guid)),
                Some(Value::Enum("Large")),
                Some(Value::Error(ValueError::InvalidBool { byte: 7 })),
            ],
        )],
    )?;

    let value: serde_json::Value = serde_json::from_str(&json)?;
    assert_eq!(
        json,
        "[{\"record path\":\"libs/r\",\"a\":false,\"b\":-8,\"c\":18446744073709551615,\"d\":1.5e-7,\"e\":\"NaN\",\"f\":\"INF\",\"g\":\"text\",\"h\":\"@item_Name\",\"i\":\"cbee34fd-6441-4000-b99f-924f7f26e19f\",\"j\":\"Large\",\"k\":{\"error\":\"the bool byte 7 is neither 0 nor 1\"}}]\n",
        "typed values"
    );
    assert!(value.is_array(), "valid JSON: {value}");
    Ok(())
}

#[test]
fn json_gives_a_dangling_reference_the_guid_of_the_missing_record() -> TestResult {
    let guid: Guid = "3cf6753c-d69c-4980-a45e-724151407096".parse()?;
    let json = written(
        ResultFormat::Json,
        &["entityClass", "list"],
        &[(
            "libs/r",
            vec![
                Some(Value::Error(ValueError::DanglingReference { guid })),
                Some(Value::Array(Box::new([Value::Error(
                    ValueError::DanglingReference { guid },
                )]))),
            ],
        )],
    )?;

    let value: serde_json::Value = serde_json::from_str(&json)?;
    let row = value.get(0).ok_or("one row")?;
    for cell in [&row["entityClass"], &row["list"][0]] {
        assert_eq!(
            cell["guid"], "3cf6753c-d69c-4980-a45e-724151407096",
            "the GUID next to the reason: {json}"
        );
        assert!(cell["error"].is_string(), "the reason stays: {json}");
    }
    Ok(())
}

#[test]
fn json_escapes_quotes_backslashes_and_control_characters() -> TestResult {
    let json = written(
        ResultFormat::Json,
        &["`say \"x\"`"],
        &[(
            "libs\\r",
            vec![Some(Value::String(
                "a\"b\\c\nd\re\tf\u{1}g\u{7f}h\u{2028}i",
            ))],
        )],
    )?;

    let value: serde_json::Value = serde_json::from_str(&json)?;
    assert_eq!(
        json,
        "[{\"record path\":\"libs\\\\r\",\"`say \\\"x\\\"`\":\"a\\\"b\\\\c\\nd\\re\\tf\\u0001g\u{7f}h\u{2028}i\"}]\n",
        "escaped"
    );
    assert_eq!(
        value
            .pointer("/0/`say \"x\"`")
            .and_then(serde_json::Value::as_str),
        Some("a\"b\\c\nd\re\tf\u{1}g\u{7f}h\u{2028}i"),
        "reads back"
    );
    Ok(())
}

#[test]
fn json_keeps_the_first_of_repeated_columns_and_csv_keeps_all() -> TestResult {
    let row = vec![
        Some(Value::Int32(1)),
        Some(Value::Int32(2)),
        Some(Value::Int32(1)),
    ];
    let json = written(
        ResultFormat::Json,
        &["a", "b", "a"],
        &[("libs/r", row.clone())],
    )?;
    let csv = written(ResultFormat::CsvRaw, &["a", "b", "a"], &[("libs/r", row)])?;

    assert_eq!(
        json, "[{\"record path\":\"libs/r\",\"a\":1,\"b\":2}]\n",
        "unique keys"
    );
    assert_eq!(csv, "record path,a,b,a\r\nlibs/r,1,2,1\r\n", "every column");
    Ok(())
}

#[test]
fn every_row_is_written_through_before_the_result_ends() -> TestResult {
    for format in [ResultFormat::Csv, ResultFormat::CsvRaw, ResultFormat::Json] {
        let mut writer = ResultWriter::new(Vec::new(), format, &columns(&["n"]))?;
        let mut sizes = vec![writer.get_ref().len()];
        for index in 0..3 {
            writer.write_row("libs/r", &[Some(Value::Int32(index))])?;
            sizes.push(writer.get_ref().len());
        }

        assert!(
            sizes
                .windows(2)
                .all(|pair| matches!(pair, [before, after] if before < after)),
            "{format:?} writes each row at once: {sizes:?}"
        );
    }
    Ok(())
}

#[test]
fn strict_csv_preserves_text_and_numbers_and_quotes_empty_fields() -> TestResult {
    let actual = written(
        ResultFormat::CsvStrict,
        &["", "header,\"text\"", "missing"],
        &[(
            "libs/r",
            vec![
                Some(Value::String("=1+1\r\n\"line\"")),
                Some(Value::Int32(-5)),
            ],
        )],
    )?;

    assert_eq!(
        actual,
        "record path,\"\",\"header,\"\"text\"\"\",missing\r\nlibs/r,\"=1+1\r\n\"\"line\"\"\",-5,\"\"\r\n",
        "strict CSV uses CRLF, no BOM and no spreadsheet prefix"
    );
    Ok(())
}

#[test]
fn strict_csv_rejects_unsupported_header_path_and_value_bytes() -> TestResult {
    let mut header = Vec::new();
    let error = ResultWriter::new(&mut header, ResultFormat::CsvStrict, &columns(&["a\tb"]))
        .expect_err("an unsupported heading is refused");
    assert_eq!(error.kind(), ErrorKind::InvalidData, "strict header error");
    assert_eq!(header, b"record path,", "no bytes of the invalid heading");

    let mut writer = ResultWriter::new(Vec::new(), ResultFormat::CsvStrict, &columns(&["a"]))?;
    let error = writer
        .write_row("libs/é", &[Some(Value::Int32(1))])
        .expect_err("a non-ASCII path is refused");
    assert_eq!(error.kind(), ErrorKind::InvalidData, "strict path error");
    assert_eq!(
        writer.get_ref().as_slice(),
        b"record path,a\r\n",
        "no bytes of the invalid record path"
    );

    let error = writer
        .write_row("libs/r", &[Some(Value::String("private,\né"))])
        .expect_err("a non-ASCII text cell is refused");
    assert_eq!(error.kind(), ErrorKind::InvalidData, "strict cell error");
    assert_eq!(
        writer.get_ref().as_slice(),
        b"record path,a\r\nlibs/r,",
        "no bytes of the invalid cell"
    );
    assert_eq!(writer.rows(), 0, "refused rows do not count as written");
    Ok(())
}
