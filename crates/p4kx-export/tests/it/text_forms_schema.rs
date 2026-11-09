// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use jsonschema::Validator;
use p4kx_extraction::{Archive, IniEncoding, Language};
use serde_json::{Value, json};
use tempfile::TempDir;

use crate::text_forms_assertions as check;
use crate::text_forms_fixture::{self as fixture, Origin, TestResult};
use crate::text_forms_schema_assertions;

#[test]
fn text_forms_schema_requires_both_sourced_forms_and_keeps_protection_by_domain() -> TestResult {
    let (_directory, archive) = schema_source()?;
    let (_, catalog) = fixture::export(&archive)?;
    let schema = p4kx_export::catalog_schema();
    let validator = jsonschema::validator_for(&schema)?;
    assert!(
        validator.is_valid(&catalog),
        "the actual canonical producer satisfies its schema"
    );
    check::forms(fixture::text(&catalog, "publicNames", "name")?, "en")?;
    check::forms(fixture::text(&catalog, "gameTexts", "description")?, "en")?;
    assert_domain_protection(&schema)?;
    assert_required_forms(&validator, &catalog)?;
    assert_scalar_rejected(&validator, &catalog)
}

fn assert_domain_protection(schema: &Value) -> TestResult {
    assert_eq!(
        schema.pointer("/$defs/CatalogTexts/properties/gameTexts/x-gameText"),
        Some(&json!(true)),
        "the protected owner context continues to protect both description forms"
    );
    assert!(
        schema
            .pointer("/$defs/CatalogTexts/properties/publicNames/x-gameText")
            .is_none(),
        "public template forms do not inherit the protected-domain annotation"
    );
    assert_public_protection(schema)
}

fn assert_required_forms(validator: &Validator, catalog: &Value) -> TestResult {
    for field in ["sourceText", "displayText"] {
        let mut missing = catalog.clone();
        missing
            .pointer_mut("/texts/publicNames/0/translations/value/en")
            .and_then(Value::as_object_mut)
            .ok_or("actual shared forms")?
            .remove(field)
            .ok_or("actual required form")?;
        assert!(
            !validator.is_valid(&missing),
            "missing {field} cannot be accepted"
        );
        let mut originless = catalog.clone();
        originless
            .pointer_mut(&format!(
                "/texts/publicNames/0/translations/value/en/{field}"
            ))
            .and_then(Value::as_object_mut)
            .ok_or("actual immediate form")?
            .remove("containsEncryptedSource")
            .ok_or("actual marker")?;
        assert!(
            !validator.is_valid(&originless),
            "each form requires its direct marker"
        );
    }
    Ok(())
}

fn assert_scalar_rejected(validator: &Validator, catalog: &Value) -> TestResult {
    let mut scalar = catalog.clone();
    let old_carrier = fixture::at(
        &scalar,
        "/texts/publicNames/0/translations/value/en/displayText",
    )?
    .clone();
    *scalar
        .pointer_mut("/texts/publicNames/0/translations/value/en")
        .ok_or("language forms")? = old_carrier;
    assert!(
        !validator.is_valid(&scalar),
        "the old sourced string carrier cannot replace the two-form language group"
    );
    Ok(())
}

fn schema_source() -> TestResult<(TempDir, Archive)> {
    let database = fixture::with_keys("@case", "@case");
    fixture::original_keys(&database, "@case", "@case", 1)?;
    let (directory, archive) = fixture::open(
        &database,
        [fixture::locale(
            Language::English,
            b"case= Raw\\nDisplay \r\n".to_vec(),
            Origin::Plain,
        )],
    )?;
    fixture::original_locale(
        &archive,
        Language::English,
        "@case",
        Some(" Raw\\nDisplay "),
        IniEncoding::Utf8,
        false,
    )?;
    Ok((directory, archive))
}

fn assert_public_protection(schema: &Value) -> TestResult {
    assert!(
        !text_forms_schema_assertions::public_is_protected(schema)?,
        "resolved public keys, translations and both shared forms stay outside protected-text annotations"
    );
    let forms = text_forms_schema_assertions::forms_pointer(schema)?;
    let mut protected = schema.clone();
    protected
        .pointer_mut(&forms)
        .and_then(Value::as_object_mut)
        .ok_or("actual resolved shared forms schema")?
        .insert("x-gameText".to_owned(), json!(true));
    assert!(
        text_forms_schema_assertions::public_is_protected(&protected)?,
        "the public-domain oracle detects an annotation added to the actual shared forms definition"
    );
    assert_public_alias_protection(schema)
}

fn assert_public_alias_protection(schema: &Value) -> TestResult {
    let mut protected = schema.clone();
    let alias = protected
        .pointer_mut("/$defs/CatalogTexts/properties/publicNames/items")
        .and_then(Value::as_object_mut)
        .ok_or("actual public canonical row alias")?;
    assert!(
        alias.get("$ref").and_then(Value::as_str).is_some(),
        "the negative changes the actual public items reference node, not a fabricated schema shape"
    );
    alias.insert("x-gameText".to_owned(), json!(true));
    assert!(
        text_forms_schema_assertions::public_is_protected(&protected)?,
        "the public-domain oracle detects protected annotations beside the actual items reference"
    );
    Ok(())
}
