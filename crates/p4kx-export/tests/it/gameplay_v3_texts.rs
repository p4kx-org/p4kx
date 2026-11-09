// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::gameplay_v3_schema as schema;
use crate::gameplay_v3_source as source;
use crate::gameplay_v3_support::{self as support, TestResult};
use crate::gameplay_v3_values as values;
use crate::text_forms_assertions as forms;
use crate::text_forms_fixture as locale;
use crate::text_forms_origin_assertions as origins;
use p4kx_extraction::{IniEncoding, Language, Value as Raw};
use p4kx_testkit::TestValue;
use serde_json::{Value, json};

const ENGLISH: &str = "  %+.2f\\n units  ";
const GERMAN: &str = "  %+.2f\\n Einheiten  ";

#[test]
fn gameplay_v3_name_and_unit_templates_share_public_text_forms_without_evaluation_or_extra_lookups()
-> TestResult {
    let database = database();
    original_keys(&database)?;
    let (_folder, archive) = original_archive(&database)?;
    let (built, catalog) = locale::export(&archive)?;
    let text = public_text(&catalog)?;
    check_forms(&catalog, text)?;
    for language in [Language::English, Language::German] {
        forms::summary(&built, language, 3, 0, 0)?;
    }
    crate::gameplay_v3_text_states::empty_and_placeholder()
}

fn check_forms(catalog: &Value, text: &Value) -> TestResult {
    let owners = [
        (0, "propertyName"),
        (0, "unitFormat"),
        (0, "nameOverrides[0].propertyName"),
    ];
    for (language, raw, display, marker, lookup) in [
        (
            Language::English,
            ENGLISH,
            "%+.2f\n units",
            Some(false),
            origins::Lookup::Source,
        ),
        (
            Language::German,
            GERMAN,
            "%+.2f\n Einheiten",
            Some(true),
            origins::Lookup::DecodedSource,
        ),
    ] {
        let form = forms::forms(text, language.code())?;
        forms::present(form, "sourceText", raw)?;
        forms::present(form, "displayText", display)?;
        let source_text = support::at(form, "/sourceText")?;
        let display_text = support::at(form, "/displayText")?;
        support::marker(source_text, marker)?;
        support::marker(display_text, marker)?;
        assert_eq!(
            origins::actual(catalog, source_text, language)?,
            origins::expected(&owners, lookup),
            "sourceText uses only actual key/winner and recorded Entry decoding"
        );
        assert_eq!(
            origins::actual(catalog, display_text, language)?,
            origins::expected(&owners, origins::Lookup::Display),
            "the display normalization has its own actual conversion"
        );
    }
    assert_eq!(
        support::at(text, "/key/value")?,
        &json!("@template"),
        "the authored key is not normalized away"
    );
    Ok(())
}

fn database() -> p4kx_testkit::DataForgeBuilder {
    schema::schema().record(
        values::record(values::FIRST, "Primary", values::owned(schema::BASE))
            .value("propertyName", TestValue::text("@template"))
            .value("unitFormat", TestValue::text("@template"))
            .value(
                "nameOverrides",
                TestValue::Array(vec![values::name_override(
                    "@template",
                    values::owned(schema::CONDITION),
                )]),
            ),
    )
}

fn original_keys(database: &p4kx_testkit::DataForgeBuilder) -> TestResult {
    source::with_tree(database, 0, |tree| {
        for field in ["propertyName", "unitFormat"] {
            assert_eq!(
                tree.root().field(field),
                Some(&Raw::Locale("@template")),
                "actual original UI Locale field"
            );
        }
        let entry = source::structure(source::array(tree.root().field("nameOverrides"))?.first())?;
        assert_eq!(
            entry.field("propertyName"),
            Some(&Raw::Locale("@template")),
            "actual override Locale"
        );
        Ok(())
    })?;
    Ok(())
}

fn original_archive(
    database: &p4kx_testkit::DataForgeBuilder,
) -> TestResult<(tempfile::TempDir, p4kx_extraction::Archive)> {
    let entries = [
        locale::locale(
            Language::English,
            format!("template={ENGLISH}\n").into_bytes(),
            locale::Origin::Plain,
        ),
        locale::locale(
            Language::German,
            locale::encoded(&format!("template={GERMAN}\n"), IniEncoding::Utf16Le)?,
            locale::Origin::Encrypted,
        ),
    ];
    let (folder, archive) = locale::open(database, entries)?;
    locale::original_locale(
        &archive,
        Language::English,
        "@template",
        Some(ENGLISH),
        IniEncoding::Utf8,
        false,
    )?;
    locale::original_locale(
        &archive,
        Language::German,
        "@template",
        Some(GERMAN),
        IniEncoding::Utf16Le,
        false,
    )?;
    Ok((folder, archive))
}

fn public_text(catalog: &Value) -> TestResult<&Value> {
    let row = support::row(catalog, 0)?;
    let id = support::at(row, "/propertyName/value")?;
    assert_eq!(
        support::at(row, "/unitFormat/value")?,
        id,
        "the unit key uses the same public domain and identifier"
    );
    assert_eq!(
        support::at(
            row,
            "/nameOverrides/value/0/value/payload/value/propertyName/value"
        )?,
        id,
        "override names use that same public registry"
    );
    let texts = support::at(catalog, "/texts/publicNames")?
        .as_array()
        .ok_or("public names")?;
    assert_eq!(
        texts.len(),
        1,
        "one exact public domain/key produces one canonical row"
    );
    assert!(
        support::at(catalog, "/texts/gameTexts")?
            .as_array()
            .ok_or("protected rows")?
            .is_empty(),
        "a UI formatting key is not reclassified as protected description text"
    );
    let text = texts.first().ok_or("actual public text row")?;
    assert_eq!(
        support::at(text, "/id/value")?,
        id,
        "the actual D references close in the text registry"
    );
    Ok(text)
}
