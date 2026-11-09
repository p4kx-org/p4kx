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
use serde_json::Value;

pub(crate) fn empty_and_placeholder() -> TestResult {
    let database = schema::schema().record(
        values::record(values::FIRST, "Primary", values::owned(schema::BASE))
            .value("propertyName", TestValue::text(""))
            .value("unitFormat", TestValue::text("@LOC_EMPTY")),
    );
    source::with_tree(&database, 0, |tree| {
        assert_eq!(
            tree.root().field("propertyName"),
            Some(&Raw::Locale("")),
            "an empty original Locale is present"
        );
        assert_eq!(
            tree.root().field("unitFormat"),
            Some(&Raw::Locale("@LOC_EMPTY")),
            "the original placeholder key is not absent"
        );
        Ok(())
    })?;
    let (_folder, archive) = locale::open(
        &database,
        [locale::locale(
            Language::English,
            b"LOC_EMPTY=stored but not consulted\n".to_vec(),
            locale::Origin::Encrypted,
        )],
    )?;
    locale::original_locale(
        &archive,
        Language::English,
        "@LOC_EMPTY",
        Some("stored but not consulted"),
        IniEncoding::Utf8,
        false,
    )?;
    let (_, catalog) = locale::export(&archive)?;
    assert_text_states(&catalog)
}

fn assert_text_states(catalog: &Value) -> TestResult {
    let row = support::row(catalog, 0)?;
    for (field, key, marker, lookup) in [
        ("propertyName", "", Some(true), origins::Lookup::Missing),
        (
            "unitFormat",
            "@LOC_EMPTY",
            Some(false),
            origins::Lookup::Placeholder,
        ),
    ] {
        let id = support::at(row, &format!("/{field}/value"))?;
        let text = support::at(catalog, "/texts/publicNames")?
            .as_array()
            .ok_or("public names")?
            .iter()
            .find(|text| text.pointer("/id/value") == Some(id))
            .ok_or("actual referenced public text")?;
        assert_eq!(
            support::at(text, "/key/value")?.as_str(),
            Some(key),
            "raw authored key state is not normalized away"
        );
        let language = forms::forms(text, "en")?;
        forms::absent(language)?;
        for name in ["sourceText", "displayText"] {
            let cell = support::at(language, &format!("/{name}"))?;
            support::marker(cell, marker)?;
            assert_eq!(
                origins::actual(catalog, cell, Language::English)?,
                origins::expected(&[(0, field)], lookup),
                "placeholder has no locale use; an empty key retains the actual available-key absence selector"
            );
        }
        assert!(
            support::at(text, "/translations/value")?
                .get("de")
                .is_none(),
            "no missing language file is fabricated"
        );
    }
    Ok(())
}
