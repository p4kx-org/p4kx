// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Writing the localisation table, through the public API.

use std::error::Error;

use p4kx_export::{Language, TableFormat, write_localization_table};
use p4kx_extraction::{IniText, TextSet};
use p4kx_testkit::GlobalIniBuilder;

type TestResult = Result<(), Box<dyn Error>>;

fn files() -> (Vec<u8>, Vec<u8>) {
    (
        GlobalIniBuilder::new()
            .text("b_key", "B, \"quoted\"")
            .text("A_key", "line\\nbreak")
            .text("only_en", "x")
            .build(),
        GlobalIniBuilder::new()
            .text("B_KEY", "B de")
            .text("only_de", "")
            .build(),
    )
}

fn written(format: TableFormat, languages: &[Language]) -> Result<(usize, String), Box<dyn Error>> {
    let (english, german) = files();
    let (en, de) = (IniText::decode(&english)?, IniText::decode(&german)?);
    let set = TextSet::new([
        (Language::English, en.index()?),
        (Language::German, de.index()?),
    ]);
    let mut out = Vec::new();
    let keys = write_localization_table(&set, languages, format, &mut out)?;
    Ok((keys, String::from_utf8(out)?))
}

#[test]
fn json_lists_each_key_once_sorted_with_null_for_a_missing_text() -> TestResult {
    let (keys, text) = written(TableFormat::Json, &[Language::English, Language::German])?;
    assert_eq!(keys, 4, "four keys");
    assert_eq!(
        text,
        concat!(
            "{\"languages\":[\"en\",\"de\"],\"entries\":[\n",
            "{\"key\":\"A_key\",\"texts\":{\"en\":\"line\\nbreak\",\"de\":null}},\n",
            "{\"key\":\"b_key\",\"texts\":{\"en\":\"B, \\\"quoted\\\"\",\"de\":\"B de\"}},\n",
            "{\"key\":\"only_de\",\"texts\":{\"en\":null,\"de\":\"\"}},\n",
            "{\"key\":\"only_en\",\"texts\":{\"en\":\"x\",\"de\":null}}\n",
            "]}\n"
        ),
        "JSON"
    );
    serde_json::from_str::<serde_json::Value>(&text)?;
    Ok(())
}

#[test]
fn csv_quotes_cells_and_leaves_missing_texts_empty() -> TestResult {
    let (keys, text) = written(TableFormat::Csv, &[Language::German, Language::English])?;
    assert_eq!(keys, 4, "four keys");
    assert_eq!(
        text,
        "\u{feff}key,de,en\r\nA_key,,\"line\nbreak\"\r\nb_key,B de,\"B, \"\"quoted\"\"\"\r\nonly_de,,\r\nonly_en,,x\r\n",
        "CSV"
    );
    Ok(())
}

fn formulas(format: TableFormat) -> Result<String, Box<dyn Error>> {
    let english = GlobalIniBuilder::new()
        .text("+key", "=1+1")
        .text("plain", "-5 items")
        .text("number", "12")
        .build();
    let text = IniText::decode(&english)?;
    let set = TextSet::new([(Language::English, text.index()?)]);
    let mut out = Vec::new();
    write_localization_table(&set, &[Language::English], format, &mut out)?;
    Ok(String::from_utf8(out)?)
}

#[test]
fn csv_neutralises_a_key_or_text_that_a_spreadsheet_would_run_as_a_formula() -> TestResult {
    assert_eq!(
        formulas(TableFormat::Csv)?,
        "\u{feff}key,en\r\n'+key,'=1+1\r\nnumber,12\r\nplain,'-5 items\r\n",
        "CSV"
    );
    Ok(())
}

#[test]
fn raw_csv_keeps_the_exact_keys_and_texts() -> TestResult {
    assert_eq!(
        formulas(TableFormat::CsvRaw)?,
        "\u{feff}key,en\r\n+key,=1+1\r\nnumber,12\r\nplain,-5 items\r\n",
        "raw CSV"
    );
    Ok(())
}

#[test]
fn strict_csv_writes_ascii_without_a_bom_and_quotes_empty_fields() -> TestResult {
    let (keys, text) = written(
        TableFormat::CsvStrict,
        &[Language::German, Language::English],
    )?;
    assert_eq!(keys, 4, "four keys");
    assert_eq!(
        text,
        "key,de,en\r\nA_key,\"\",\"line\nbreak\"\r\nb_key,B de,\"B, \"\"quoted\"\"\"\r\nonly_de,\"\",\"\"\r\nonly_en,\"\",x\r\n",
        "strict CSV"
    );
    Ok(())
}

#[test]
fn keys_without_a_text_in_the_chosen_languages_are_left_out() -> TestResult {
    let (keys, text) = written(TableFormat::Csv, &[Language::German])?;
    assert_eq!(keys, 2, "two keys have a German line");
    assert_eq!(text, "\u{feff}key,de\r\nb_key,B de\r\nonly_de,\r\n", "CSV");
    Ok(())
}

#[test]
fn an_empty_selection_is_a_valid_empty_table() -> TestResult {
    let (keys, text) = written(TableFormat::Json, &[Language::French])?;
    assert_eq!(keys, 0, "no keys");
    assert_eq!(text, "{\"languages\":[\"fr\"],\"entries\":[]}\n", "JSON");
    Ok(())
}
