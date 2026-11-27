// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{IniEncoding, Language};
use serde_json::{Value, json};

use crate::commodity_v3_source_inventory;
use crate::text_forms_assertions as check;
use crate::text_forms_fixture::{self as fixture, Origin, TestResult};
use crate::text_forms_origin_assertions::{self as origins, Lookup};

#[test]
fn text_forms_separate_original_entry_decoding_from_actual_display_conversion() -> TestResult {
    let raw = " \t%.3f widgets\\n%+.1f %% ";
    for encoding in [
        IniEncoding::Utf8,
        IniEncoding::Utf8Bom,
        IniEncoding::Utf16Le,
        IniEncoding::Utf16Be,
    ] {
        decoding_case(
            raw,
            fixture::encoded(&format!("case={raw}\r\n"), encoding)?,
            encoding,
            false,
            encoding != IniEncoding::Utf8,
        )?;
    }
    let mut lossy = b"unrelated=".to_vec();
    lossy.push(0xff);
    lossy.extend_from_slice(format!("\r\ncase={raw}\r\n").as_bytes());
    decoding_case(raw, lossy, IniEncoding::Utf8, true, true)?;
    Ok(())
}

fn decoding_case(
    raw: &str,
    bytes: Vec<u8>,
    encoding: IniEncoding,
    lossy: bool,
    decoded: bool,
) -> TestResult {
    let database = fixture::with_keys("@case", "@LOC_EMPTY");
    fixture::original_keys(&database, "@case", "@LOC_EMPTY", 1)?;
    let (_directory, archive) = fixture::open(
        &database,
        [fixture::locale(Language::English, bytes, Origin::Plain)],
    )?;
    fixture::original_locale(
        &archive,
        Language::English,
        "@case",
        Some(raw),
        encoding,
        lossy,
    )?;
    let (_, catalog) = fixture::export(&archive)?;
    assert_decoded_forms(&catalog, raw, decoded)?;
    if encoding == IniEncoding::Utf8 && !lossy {
        let forms = check::forms(fixture::text(&catalog, "publicNames", "name")?, "en")?;
        assert_negative_inputs(&catalog, fixture::at(forms, "/sourceText")?)?;
    }
    commodity_v3_source_inventory::reference_closure(&catalog)
}

fn assert_decoded_forms(catalog: &Value, raw: &str, decoded: bool) -> TestResult {
    let forms = check::forms(fixture::text(catalog, "publicNames", "name")?, "en")?;
    check::present(forms, "sourceText", raw)?;
    check::present(forms, "displayText", "%.3f widgets\n%+.1f %%")?;
    let source_kind = if decoded {
        Lookup::DecodedSource
    } else {
        Lookup::Source
    };
    assert_eq!(
        origins::actual(
            catalog,
            fixture::at(forms, "/sourceText")?,
            Language::English
        )?,
        origins::expected(&[(0, "Localization.Name")], source_kind),
        "source text has exactly its original key selector and winning locale inputs, with only justified decoder conversion"
    );
    assert_eq!(
        origins::actual(
            catalog,
            fixture::at(forms, "/displayText")?,
            Language::English
        )?,
        origins::expected(&[(0, "Localization.Name")], Lookup::Display),
        "display text adds only the actual winning-input conversion, without unrelated body or header uses"
    );
    Ok(())
}

#[test]
fn text_forms_keep_direct_crypto_markers_per_form_without_other_locale_taint() -> TestResult {
    for (origin, expected) in [
        (Origin::Plain, json!(false)),
        (Origin::Encrypted, json!(true)),
        (Origin::Unknown, Value::Null),
    ] {
        crypto_case(origin, &expected)?;
    }
    Ok(())
}

fn crypto_case(origin: Origin, expected: &Value) -> TestResult {
    let database = fixture::with_keys("@case", "@LOC_EMPTY");
    fixture::original_keys(&database, "@case", "@LOC_EMPTY", 1)?;
    let (_directory, archive) = fixture::open(
        &database,
        [
            fixture::locale(
                Language::English,
                b"case= %.4f cycles \r\n".to_vec(),
                origin,
            ),
            fixture::locale(
                Language::German,
                b"other=Verschluesselt\r\n".to_vec(),
                Origin::Encrypted,
            ),
        ],
    )?;
    fixture::original_locale(
        &archive,
        Language::English,
        "@case",
        Some(" %.4f cycles "),
        IniEncoding::Utf8,
        false,
    )?;
    let (_, catalog) = fixture::export(&archive)?;
    assert_crypto_forms(&catalog, expected)?;
    assert_dcb_fields(&catalog)?;
    commodity_v3_source_inventory::reference_closure(&catalog)
}

fn assert_crypto_forms(catalog: &Value, expected: &Value) -> TestResult {
    let row = fixture::text(catalog, "publicNames", "name")?;
    let forms = check::forms(row, "en")?;
    for (field, kind) in [
        ("sourceText", Lookup::Source),
        ("displayText", Lookup::Display),
    ] {
        let carrier = fixture::at(forms, &format!("/{field}"))?;
        check::direct(carrier)?;
        assert_eq!(
            fixture::at(carrier, "/containsEncryptedSource")?,
            expected,
            "the actual English source determines the explicit {field} marker"
        );
        assert_eq!(
            origins::actual(catalog, carrier, Language::English)?,
            origins::expected(&[(0, "Localization.Name")], kind),
            "each form retains the exact original owner, key field and English lookup contributions"
        );
    }
    assert_missing_forms(catalog, row)
}

fn assert_missing_forms(catalog: &Value, row: &Value) -> TestResult {
    let missing = check::forms(row, "de")?;
    check::absent(missing)?;
    for field in ["sourceText", "displayText"] {
        let carrier = fixture::at(missing, &format!("/{field}"))?;
        assert_eq!(
            origins::actual(catalog, carrier, Language::German)?,
            origins::expected(&[(0, "Localization.Name")], Lookup::Missing),
            "an available locale lacking the key contributes an actual absence selector and no text payload or conversion"
        );
        assert_eq!(
            fixture::at(carrier, "/containsEncryptedSource")?,
            &json!(true),
            "the missing German key retains its actual encrypted absence proof"
        );
    }
    Ok(())
}

fn assert_dcb_fields(catalog: &Value) -> TestResult {
    for field in ["/guid", "/code", "/localization/value/name"] {
        assert_eq!(
            fixture::at(
                crate::manufacturer_v3_fixture::row(catalog)?,
                &format!("{field}/containsEncryptedSource")
            )?,
            &json!(false),
            "loaded locales do not supply the original DCB field"
        );
    }
    Ok(())
}

fn assert_negative_inputs(catalog: &Value, source: &Value) -> TestResult {
    let extra_id = field_use(catalog, "Code", "payload")?;
    let extra = with_extra_input(source, extra_id)?;
    let expected = origins::expected(&[(0, "Localization.Name")], Lookup::Source);
    assert_ne!(
        origins::actual(catalog, &extra, Language::English)?,
        expected,
        "the complete-origin oracle rejects a real unrelated DCB Payload without a text or marker change"
    );
    assert_unrelated_conversion(catalog, source)?;
    let changed = wrong_lookup_selector(catalog, source)?;
    assert_ne!(
        origins::actual(&changed, source, Language::English)?,
        expected,
        "the complete-origin oracle resolves and rejects an actual wrong authored-key selector"
    );
    Ok(())
}

fn with_extra_input(source: &Value, reference: Value) -> TestResult<Value> {
    let mut extra = source.clone();
    extra
        .get_mut("sourceUsesRefs")
        .and_then(Value::as_array_mut)
        .ok_or("actual form input references")?
        .push(reference);
    assert_same_value(source, &extra)?;
    Ok(extra)
}

fn assert_unrelated_conversion(catalog: &Value, source: &Value) -> TestResult {
    let owner = crate::manufacturer_v3_fixture::row(catalog)?;
    let references = fixture::at(owner, "/className/sourceUsesRefs")?
        .as_array()
        .ok_or("actual class-name source references")?;
    let inventory = fixture::at(catalog, "/provenance/uses")?
        .as_array()
        .ok_or("actual retained source inventory")?;
    let mut reference = None;
    for id in references {
        if origins::unique(inventory, id)?
            .get("role")
            .and_then(Value::as_str)
            == Some("conversion")
        {
            reference = Some(id.clone());
            break;
        }
    }
    let extra = with_extra_input(
        source,
        reference.ok_or("actual unrelated class-name Conversion")?,
    )?;
    let rejected = origins::actual(catalog, &extra, Language::English).map_or(true, |actual| {
        actual != origins::expected(&[(0, "Localization.Name")], Lookup::Source)
    });
    assert!(
        rejected,
        "the complete-origin oracle rejects a real unrelated Conversion without changing the text or its markers"
    );
    Ok(())
}

fn field_use(catalog: &Value, field: &str, role: &str) -> TestResult<Value> {
    let inventory = fixture::at(catalog, "/provenance/uses")?
        .as_array()
        .ok_or("actual input inventory")?;
    let used = inventory
        .iter()
        .find(|used| {
            used.pointer("/location/type").and_then(Value::as_str) == Some("dataForge")
                && used
                    .pointer("/location/recordIndex")
                    .and_then(Value::as_u64)
                    == Some(0)
                && used.pointer("/location/field").and_then(Value::as_str) == Some(field)
                && used.get("role").and_then(Value::as_str) == Some(role)
        })
        .ok_or("real existing original-field source use")?;
    Ok(fixture::at(used, "/id")?.clone())
}

fn wrong_lookup_selector(catalog: &Value, source: &Value) -> TestResult<Value> {
    let selector = field_use(catalog, "Localization.Description", "selector")?;
    let inventory = fixture::at(catalog, "/provenance/uses")?
        .as_array()
        .ok_or("actual source uses")?;
    let references = fixture::at(source, "/sourceUsesRefs")?
        .as_array()
        .ok_or("actual form references")?;
    let mut id = None;
    for reference in references {
        let used = origins::unique(inventory, reference)?;
        if used.pointer("/location/type").and_then(Value::as_str) == Some("localizationLookup")
            && used.get("role").and_then(Value::as_str) == Some("payload")
        {
            id = Some(reference.clone());
            break;
        }
    }
    let id = id.ok_or("actual winning lookup Payload")?;
    let mut changed = catalog.clone();
    let used = changed
        .pointer_mut("/provenance/uses")
        .and_then(Value::as_array_mut)
        .ok_or("actual mutable source inventory")?
        .iter_mut()
        .find(|used| used.get("id") == Some(&id))
        .ok_or("actual retained lookup use")?;
    *used
        .pointer_mut("/location/selector")
        .ok_or("actual authored-key selector reference")? = selector;
    Ok(changed)
}

fn assert_same_value(original: &Value, changed: &Value) -> TestResult {
    for field in [
        "state",
        "value",
        "containsEncryptedSource",
        "sourceSetComplete",
    ] {
        assert_eq!(
            fixture::at(original, &format!("/{field}"))?,
            fixture::at(changed, &format!("/{field}"))?,
            "origin-only mutations preserve the original {field}"
        );
    }
    Ok(())
}
