// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_extraction::{IniEncoding, Language};
use serde_json::{Value, json};

use crate::text_forms_assertions as check;
use crate::text_forms_fixture::{self as fixture, Origin, TestResult};
use crate::text_forms_origin_assertions::{self as origins, Lookup};

#[test]
fn text_forms_distinguish_empty_padding_missing_key_and_missing_language_file() -> TestResult {
    for (input, raw, display) in [
        ("case=\r\n", Some(""), Some("")),
        ("case= \t\u{2003}\r\n", Some(" \t\u{2003}"), Some("")),
        ("other=Unrelated\r\n", None, None),
    ] {
        state_case(input, raw, display)?;
    }
    Ok(())
}

fn state_case(input: &str, raw: Option<&str>, display: Option<&str>) -> TestResult {
    let database = fixture::with_keys("@case", "@LOC_EMPTY");
    fixture::original_keys(&database, "@case", "@LOC_EMPTY", 1)?;
    let (_directory, archive) = fixture::open(
        &database,
        [
            fixture::locale(Language::English, input.as_bytes().to_vec(), Origin::Plain),
            fixture::locale(
                Language::German,
                b"other=Andere\r\n".to_vec(),
                Origin::Plain,
            ),
        ],
    )?;
    fixture::original_locale(
        &archive,
        Language::English,
        "@case",
        raw,
        IniEncoding::Utf8,
        false,
    )?;
    fixture::original_locale(
        &archive,
        Language::German,
        "@case",
        None,
        IniEncoding::Utf8,
        false,
    )?;
    let (built, catalog) = fixture::export(&archive)?;
    assert_state_values(&catalog, raw, display)?;
    check::summary(&built, Language::English, 0, 1, 1)?;
    check::summary(&built, Language::German, 0, 1, 1)?;
    Ok(())
}

fn assert_state_values(catalog: &Value, raw: Option<&str>, display: Option<&str>) -> TestResult {
    let row = fixture::text(catalog, "publicNames", "name")?;
    let languages = fixture::at(row, "/translations/value")?
        .as_object()
        .ok_or("actual available language map")?;
    assert_eq!(
        languages
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["de", "en"]),
        "no missing language file is invented"
    );
    let forms = check::forms(row, "en")?;
    if let (Some(raw), Some(display)) = (raw, display) {
        check::present(forms, "sourceText", raw)?;
        check::present(forms, "displayText", display)?;
    } else {
        check::absent(forms)?;
    }
    assert_form_inputs(
        catalog,
        forms,
        Language::English,
        "Localization.Name",
        if raw.is_some() {
            Lookup::Source
        } else {
            Lookup::Missing
        },
    )?;
    check::absent(check::forms(row, "de")?)?;
    assert_form_inputs(
        catalog,
        check::forms(row, "de")?,
        Language::German,
        "Localization.Name",
        Lookup::Missing,
    )?;
    Ok(())
}

#[test]
fn text_forms_keep_placeholder_and_empty_keys_without_fabricating_locale_payloads() -> TestResult {
    let database = fixture::with_keys("@LOC_EMPTY", "");
    fixture::original_keys(&database, "@LOC_EMPTY", "", 1)?;
    let (_directory, archive) = fixture::open(
        &database,
        [fixture::locale(
            Language::English,
            b"LOC_EMPTY=Actual stored text must remain unused\r\n".to_vec(),
            Origin::Encrypted,
        )],
    )?;
    fixture::original_locale(
        &archive,
        Language::English,
        "@LOC_EMPTY",
        Some("Actual stored text must remain unused"),
        IniEncoding::Utf8,
        false,
    )?;
    fixture::original_locale(
        &archive,
        Language::English,
        "",
        None,
        IniEncoding::Utf8,
        false,
    )?;
    let (built, catalog) = fixture::export(&archive)?;
    assert_placeholder(&catalog)?;
    assert_empty_key(&catalog)?;
    check::summary(&built, Language::English, 0, 1, 1)?;
    Ok(())
}

fn assert_placeholder(catalog: &Value) -> TestResult {
    let placeholder = fixture::text(catalog, "publicNames", "name")?;
    assert_eq!(
        fixture::at(placeholder, "/id/value")?,
        &json!("name:10:@LOC_EMPTY"),
        "the authored placeholder keeps its original public ID"
    );
    assert_eq!(
        fixture::at(placeholder, "/key/value")?,
        &json!("@LOC_EMPTY"),
        "the placeholder key is not normalized or replaced"
    );
    let forms = check::forms(placeholder, "en")?;
    check::absent(forms)?;
    for field in ["sourceText", "displayText"] {
        let value = fixture::at(forms, &format!("/{field}"))?;
        assert_eq!(
            fixture::at(value, "/containsEncryptedSource")?,
            &json!(false),
            "a placeholder never consults the loaded encrypted locale"
        );
        assert_eq!(
            origins::actual(catalog, value, Language::English)?,
            origins::expected(&[(0, "Localization.Name")], Lookup::Placeholder),
            "a placeholder uses only the actual authored key selector, with no winning locale payload or conversion"
        );
    }
    Ok(())
}

fn assert_empty_key(catalog: &Value) -> TestResult {
    let empty = fixture::text(catalog, "gameTexts", "description")?;
    assert_eq!(
        fixture::at(empty, "/id/value")?,
        &json!("text:0:"),
        "an empty authored key retains its exact protected ID"
    );
    assert_eq!(
        fixture::at(empty, "/key/value")?,
        &json!(""),
        "present empty Locale keys are not treated as missing fields"
    );
    let forms = check::forms(empty, "en")?;
    check::absent(forms)?;
    assert_form_inputs(
        catalog,
        forms,
        Language::English,
        "Localization.Description",
        Lookup::Missing,
    )?;
    for field in ["sourceText", "displayText"] {
        assert_eq!(
            fixture::at(forms, &format!("/{field}/containsEncryptedSource"))?,
            &json!(true),
            "the actual encrypted available-key absence proof remains used"
        );
    }
    assert!(
        fixture::at(empty, "/translations/value")?
            .get("de")
            .is_none(),
        "a missing language file cannot create a forms entry"
    );
    Ok(())
}

fn assert_form_inputs(
    catalog: &Value,
    forms: &Value,
    language: Language,
    field: &str,
    source: Lookup,
) -> TestResult {
    for form in ["sourceText", "displayText"] {
        let expected = if matches!(source, Lookup::Source) && form == "displayText" {
            Lookup::Display
        } else {
            source
        };
        assert_eq!(
            origins::actual(catalog, fixture::at(forms, &format!("/{form}"))?, language)?,
            origins::expected(&[(0, field)], expected),
            "present/empty/missing forms retain only their actual original owner and language-lookup contributions"
        );
    }
    Ok(())
}
