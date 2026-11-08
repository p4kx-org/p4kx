// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Public exchange follows the generated schema and typed canonical text references.

use p4kx_export::{build_catalog, catalog_schema};
use p4kx_testkit::{TestRecord, TestValue};
use serde_json::Value;

use crate::catalog_text_exchange_support::{public_projection, reference_closure};
use crate::resource_origin_fixture::{self as fixture, TestResult};

const PRIVATE_KEY: &str = "@fixture_description";
const PRIVATE_LITERAL: &str = "PRIVATE literal without a locale key";
const PRIVATE_TEXT: &str = "Protected description";

fn catalog() -> TestResult<Value> {
    let database = fixture::database(1.0)
        .record(resource(4, "Shared", "@fixture_name", "@fixture_name"))
        .record(resource(5, "Bare", "fixture_name", PRIVATE_LITERAL));
    let (_directory, archive) = fixture::open(database.build(), false)?;
    let built = build_catalog(&archive, &fixture::options())?;
    Ok(serde_json::from_slice(built.json())?)
}

fn resource(guid: u128, name: &str, display: &str, description: &str) -> TestRecord {
    TestRecord::new(
        guid,
        "ResourceType",
        format!("ResourceType.{name}"),
        format!("libs/foundry/records/resourcetypedatabase/{name}.xml"),
    )
    .value("displayName", TestValue::text(display))
    .value("description", TestValue::text(description))
}

fn row<'a>(catalog: &'a Value, class_name: &str) -> TestResult<&'a Value> {
    catalog
        .pointer("/definitions/resourceTypes/rows")
        .and_then(Value::as_array)
        .and_then(|rows| {
            rows.iter().find(|row| {
                row.pointer("/className/value").and_then(Value::as_str) == Some(class_name)
            })
        })
        .ok_or_else(|| "resource row missing".into())
}

fn text<'a>(catalog: &'a Value, domain: &str, id: &Value) -> TestResult<&'a Value> {
    catalog
        .get("texts")
        .and_then(|texts| texts.get(domain))
        .and_then(Value::as_array)
        .and_then(|rows| rows.iter().find(|row| row.pointer("/id/value") == Some(id)))
        .ok_or_else(|| "canonical text reference is dangling".into())
}

#[test]
fn generated_game_text_policy_removes_private_reference_closure_and_keeps_shared_public_names()
-> TestResult {
    let member = catalog()?;
    let schema = catalog_schema();
    let validator = jsonschema::validator_for(&schema)?;
    assert!(
        validator.is_valid(&member),
        "actual member catalog validates before exchange"
    );
    let [public_refs, game_refs] = reference_closure(&member, &schema)?;
    assert!(
        public_refs > 0 && game_refs > 0,
        "both typed domains were traversed"
    );
    let shared = row(&member, "Shared")?;
    assert_ne!(
        shared["name"]["value"], shared["description"]["value"],
        "equal source keys use distinct typed domains"
    );
    let name = text(&member, "publicNames", &shared["name"]["value"])?;
    let description = text(&member, "gameTexts", &shared["description"]["value"])?;
    assert_eq!(
        name["translations"]["value"]["de"]["value"],
        description["translations"]["value"]["de"]["value"],
        "source bytes may be shared"
    );
    assert_eq!(
        description["translations"]["value"]["de"]["containsEncryptedSource"], false,
        "plain encryption origin is not public authorization"
    );

    let public = public_projection(&member, &schema)?;
    assert!(
        public["texts"].get("gameTexts").is_none(),
        "protected canonical rows are removed"
    );
    let public_shared = row(&public, "Shared")?;
    assert!(
        public_shared.get("description").is_none(),
        "protected owner reference is removed"
    );
    assert_eq!(
        public_shared["name"], shared["name"],
        "public short name survives"
    );
    assert_eq!(
        text(&public, "publicNames", &public_shared["name"]["value"])?,
        name,
        "public canonical row survives intact"
    );
    let [public_refs, game_refs] = reference_closure(&public, &schema)?;
    assert!(public_refs > 0, "remaining public references resolve");
    assert_eq!(game_refs, 0, "no protected reference remains reachable");
    assert_private_content_is_confined_to_members(&member, &public)?;
    Ok(())
}

fn assert_private_content_is_confined_to_members(member: &Value, public: &Value) -> TestResult {
    let serialized = serde_json::to_string(public)?;
    for forbidden in [PRIVATE_KEY, PRIVATE_LITERAL, PRIVATE_TEXT] {
        assert!(
            !serialized.contains(forbidden),
            "private-only content is absent: {forbidden}"
        );
    }
    let provenance = serde_json::to_string(&member["provenance"])?;
    assert!(
        !provenance.contains(PRIVATE_KEY) && !provenance.contains(PRIVATE_LITERAL),
        "unmarked source IDs and locations contain selector addresses, not protected strings"
    );
    assert!(
        serde_json::to_string(member)?.contains(PRIVATE_TEXT),
        "members retain the protected source text"
    );
    Ok(())
}

#[test]
fn successful_bare_keys_and_unresolved_literal_looking_fields_keep_existing_semantics() -> TestResult
{
    let member = catalog()?;
    let bare = row(&member, "Bare")?;
    let name = text(&member, "publicNames", &bare["name"]["value"])?;
    assert_eq!(
        name["key"]["value"], "fixture_name",
        "bare source key is unchanged"
    );
    assert_eq!(
        name["translations"]["value"]["en"]["value"], "English winner",
        "the existing INI winner resolves a bare key"
    );
    let description = text(&member, "gameTexts", &bare["description"]["value"])?;
    assert_eq!(
        description["key"]["value"], PRIVATE_LITERAL,
        "original field remains protected"
    );
    assert_eq!(
        description["translations"]["value"]["en"]["state"], "absent",
        "a missing bare key is not echoed as a literal translation"
    );
    Ok(())
}
