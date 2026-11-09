// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{IniEncoding, Language};
use serde_json::{Value, json};

use crate::text_forms_assertions as check;
use crate::text_forms_fixture::{self as fixture, Origin, TestResult};
use crate::text_forms_origin_assertions::{self as origins, Lookup};

#[test]
fn text_forms_keep_raw_template_bytes_and_the_original_display_value() -> TestResult {
    let raw = " \t%.3f widgets\\n%+.1f %% \u{2003}";
    let database = fixture::with_keys("@format", "@LOC_EMPTY");
    fixture::original_keys(&database, "@format", "@LOC_EMPTY", 1)?;
    let bytes = fixture::encoded(&format!("format={raw}\r\n"), IniEncoding::Utf8)?;
    let (_directory, archive) = fixture::open(
        &database,
        [fixture::locale(Language::English, bytes, Origin::Plain)],
    )?;
    fixture::original_locale(
        &archive,
        Language::English,
        "@format",
        Some(raw),
        IniEncoding::Utf8,
        false,
    )?;
    let (built, catalog) = fixture::export(&archive)?;
    let row = fixture::text(&catalog, "publicNames", "name")?;
    assert_eq!(
        fixture::at(row, "/id/value")?,
        &json!("name:7:@format"),
        "the public template keeps its original byte-length-based identifier"
    );
    assert_eq!(
        fixture::at(row, "/key/value")?,
        &json!("@format"),
        "the canonical key preserves the exact authored Locale spelling"
    );
    let forms = check::forms(row, "en")?;
    check::present(forms, "sourceText", raw)?;
    check::present(forms, "displayText", "%.3f widgets\n%+.1f %%")?;
    assert_inputs(&catalog, forms, &[(0, "Localization.Name")])?;
    check::summary(&built, Language::English, 1, 0, 1)?;
    Ok(())
}

#[test]
fn text_forms_share_one_public_id_but_count_each_original_lookup_once() -> TestResult {
    let database = fixture::repeated("@shared");
    fixture::original_keys(&database, "@shared", "@LOC_EMPTY", 2)?;
    let (_directory, archive) = fixture::open(
        &database,
        [fixture::locale(
            Language::English,
            b"shared= \tShared\\nvalue \r\n".to_vec(),
            Origin::Plain,
        )],
    )?;
    fixture::original_locale(
        &archive,
        Language::English,
        "@shared",
        Some(" \tShared\\nvalue "),
        IniEncoding::Utf8,
        false,
    )?;
    let (built, catalog) = fixture::export(&archive)?;
    let public = fixture::at(&catalog, "/texts/publicNames")?
        .as_array()
        .ok_or("public pool")?;
    assert_eq!(
        public.len(),
        1,
        "both original owners reference one canonical public row"
    );
    assert_eq!(
        fixture::at(fixture::text(&catalog, "publicNames", "name")?, "/id/value")?,
        &json!("name:7:@shared"),
        "the shared key keeps one unchanged public identifier"
    );
    check::summary(&built, Language::English, 2, 0, 2)?;
    let forms = check::forms(public.first().ok_or("shared row")?, "en")?;
    check::present(forms, "sourceText", " \tShared\\nvalue ")?;
    check::present(forms, "displayText", "Shared\nvalue")?;
    assert_shared_references(&catalog)?;
    assert_inputs(
        &catalog,
        forms,
        &[(0, "Localization.Name"), (1, "Localization.Name")],
    )?;
    Ok(())
}

#[test]
fn text_forms_preserve_domain_ids_and_the_actual_plain_suffix_lookup_winner() -> TestResult {
    let database = fixture::with_keys("@MiXeD", "@MiXeD");
    fixture::original_keys(&database, "@MiXeD", "@MiXeD", 1)?;
    let (_directory, archive) = fixture::open(
        &database,
        [fixture::locale(
            Language::English,
            b" mixed ,P=Ignored\r\nMIXED= \tPlain \r\n".to_vec(),
            Origin::Plain,
        )],
    )?;
    fixture::original_locale(
        &archive,
        Language::English,
        "@MiXeD",
        Some(" \tPlain "),
        IniEncoding::Utf8,
        false,
    )?;
    let (built, catalog) = fixture::export(&archive)?;
    for (domain, field, expected_id, input) in [
        ("publicNames", "name", "name:6:@MiXeD", "Localization.Name"),
        (
            "gameTexts",
            "description",
            "text:6:@MiXeD",
            "Localization.Description",
        ),
    ] {
        let row = fixture::text(&catalog, domain, field)?;
        assert_eq!(
            fixture::at(row, "/id/value")?,
            &json!(expected_id),
            "the same key retains its distinct original {domain} identifier"
        );
        assert_eq!(
            fixture::at(row, "/key/value")?,
            &json!("@MiXeD"),
            "lookup case folding does not rewrite the original authored key"
        );
        let forms = check::forms(row, "en")?;
        check::present(forms, "sourceText", " \tPlain ")?;
        check::present(forms, "displayText", "Plain")?;
        assert_inputs(&catalog, forms, &[(0, input)])?;
    }
    check::summary(&built, Language::English, 2, 0, 0)?;
    Ok(())
}

fn assert_inputs(catalog: &Value, forms: &Value, owners: &[(u64, &str)]) -> TestResult {
    for (form, expected) in [
        ("sourceText", Lookup::Source),
        ("displayText", Lookup::Display),
    ] {
        assert_eq!(
            origins::actual(
                catalog,
                fixture::at(forms, &format!("/{form}"))?,
                Language::English
            )?,
            origins::expected(owners, expected),
            "the shared text row retains every actual authored key occurrence and only its matching original lookup inputs"
        );
    }
    Ok(())
}

fn assert_shared_references(catalog: &Value) -> TestResult {
    let owners = crate::manufacturer_v3_fixture::rows(catalog)?;
    assert_eq!(
        owners.len(),
        2,
        "both original physical owners remain exported"
    );
    for owner in owners {
        assert_eq!(
            fixture::at(owner, "/localization/value/name/value")?,
            &json!("name:7:@shared"),
            "both physical owner references retain the one unchanged public identifier"
        );
    }
    Ok(())
}
