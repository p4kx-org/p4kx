// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use schemars::{JsonSchema, SchemaGenerator};
use serde_json::Value;

use super::{CanonicalText, GameTextRef, PublicNameRef};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn adding_a_text_domain_preserves_existing_definition_references_and_fields() -> TestResult {
    let mut initial = SchemaGenerator::default();
    let expected = initial.subschema_for::<CanonicalText<'static, PublicNameRef>>();
    let name = CanonicalText::<'static, PublicNameRef>::schema_name();
    let definition = initial
        .definitions()
        .get(name.as_ref())
        .ok_or("initial text definition")?;
    let mut expanded = SchemaGenerator::default();
    expanded.subschema_for::<CanonicalText<'static, GameTextRef>>();
    let actual = expanded.subschema_for::<CanonicalText<'static, PublicNameRef>>();
    assert_eq!(
        actual, expected,
        "adding text domains must not renumber existing canonical text references"
    );
    assert_eq!(
        expanded.definitions().get(name.as_ref()),
        Some(definition),
        "adding text domains must preserve existing typed text fields"
    );
    Ok(())
}

#[test]
fn public_and_protected_text_references_remain_separate() -> TestResult {
    let mut generator = SchemaGenerator::default();
    let public = generator.subschema_for::<CanonicalText<'static, PublicNameRef>>();
    let protected = generator.subschema_for::<CanonicalText<'static, GameTextRef>>();
    assert_ne!(
        public, protected,
        "public names and protected descriptions retain separate schema identities"
    );
    let public_name = public
        .get("$ref")
        .and_then(Value::as_str)
        .and_then(|reference| reference.strip_prefix("#/$defs/"))
        .ok_or("public text reference")?;
    let protected_name = protected
        .get("$ref")
        .and_then(Value::as_str)
        .and_then(|reference| reference.strip_prefix("#/$defs/"))
        .ok_or("protected text reference")?;
    let public = generator
        .definitions()
        .get(public_name)
        .ok_or("public text definition")?;
    let protected = generator
        .definitions()
        .get(protected_name)
        .ok_or("protected text definition")?;
    assert_ne!(
        public.pointer("/properties/id/$ref"),
        protected.pointer("/properties/id/$ref"),
        "the two domains retain their different typed reference carriers"
    );
    let game_text = generator
        .definitions()
        .get(GameTextRef::schema_name().as_ref())
        .ok_or("protected text reference")?;
    assert_eq!(
        game_text.get("x-gameText").and_then(Value::as_bool),
        Some(true),
        "schema naming must retain the protected-text annotation"
    );
    Ok(())
}
