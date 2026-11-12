// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Exactly the game-text fields of the catalog carry `x-gameText` in its JSON Schema.

use std::collections::BTreeSet;

use serde_json::Value;

const GAME_TEXT: [&str; 28] = [
    "description",
    "displayDescription",
    "gameTexts",
    "displayName",
    "displayType",
    "title",
    "missionType",
    "typeName",
    "contractor",
    "place",
    "processName",
    "depositName",
    "markerLabel",
    "shortDescription",
    "longDescription",
    "titleOverride",
    "descriptionOverride",
    "hudObjectiveText",
    "hudTitle",
    "missionGiver",
    "commsChannelName",
    "endReason",
    "givenName",
    "givenLastName",
    "givenNickName",
    "objectiveLongDescription",
    "objectiveShortDescription",
    "causeOfDeath",
];

struct Property<'schema> {
    owner: &'schema str,
    name: &'schema str,
    schema: &'schema Value,
}

fn collect_properties<'schema>(
    node: &'schema Value,
    owner: &'schema str,
    found: &mut Vec<Property<'schema>>,
) {
    match node {
        Value::Object(map) => {
            if let Some(properties) = map.get("properties").and_then(Value::as_object) {
                for (name, value) in properties {
                    found.push(Property {
                        owner,
                        name,
                        schema: value,
                    });
                    collect_properties(value, owner, found);
                }
            }
            for (name, value) in map {
                if name != "properties" && name != "$defs" {
                    collect_properties(value, owner, found);
                }
            }
        }
        Value::Array(items) => {
            for value in items {
                collect_properties(value, owner, found);
            }
        }
        _ => {}
    }
}

#[test]
fn exactly_the_game_text_fields_carry_the_game_text_annotation() {
    let schema = p4kx_export::catalog_schema();
    let definitions = schema
        .get("$defs")
        .and_then(Value::as_object)
        .expect("catalog definitions");
    let mut properties = Vec::new();
    collect_properties(&schema, "Catalog", &mut properties);
    for (owner, definition) in definitions {
        if !matches!(
            owner.as_str(),
            "SourceText" | "JsonString" | "InventoryCandidate"
        ) {
            collect_properties(definition, owner, &mut properties);
        }
    }

    let mut marked = 0;
    for Property {
        owner,
        name,
        schema: field,
    } in properties
    {
        let carries = field.get("x-gameText") == Some(&Value::Bool(true));
        let expected = expected_annotation(owner, name, field, &schema);
        assert_eq!(carries, expected, "x-gameText on {owner}.{name}: {field}");
        marked += usize::from(carries);
    }
    assert_eq!(
        marked, 73,
        "every protected field in the canonical schema is accounted for"
    );
}

fn expected_annotation(owner: &str, name: &str, field: &Value, schema: &Value) -> bool {
    match (owner, name) {
        (
            "RecoveryNotifications",
            "itemBricked_Title"
            | "itemBricked_Body"
            | "vehicleBrickedWithItems_Body"
            | "itemBricking_Title"
            | "itemBricking_Body"
            | "vehicleBrickingWithItems_Body"
            | "onBrickedItemUsed_Title"
            | "onBrickedItemUsed_CannotEquip"
            | "onBrickedItemUsed_CannotFire"
            | "onBrickedItemUsed_CannotUse",
        )
        | ("BlueprintPresentation", "name")
        | ("StandingDefinition", "perkDescription")
        | ("FactionReputationDefinition", "background")
        | ("InfractionParameters", "graceWarnings")
        | (
            "StarmapObjectDefinition",
            "name" | "description" | "callout1" | "callout2" | "callout3",
        )
        | ("TemplateLocalization", "text")
        | ("ProjectedMissionText", "key" | "translations") => true,
        ("SharedInteraction", "displayType") => false,
        ("BlackboardPropertyBody", "value") => {
            field.get("$ref").and_then(Value::as_str) == Some("#/$defs/SourcedValueOfGameTextRef")
        }
        ("BrokerProjection" | "TemplateProjection", "title") => {
            assert!(
                references(
                    field,
                    schema,
                    "#/$defs/SourceValueRef",
                    &mut BTreeSet::new()
                ),
                "{owner}.{name} is navigation to the actual source occurrence"
            );
            false
        }
        _ => GAME_TEXT.contains(&name) && !is_only_public_name(schema, field),
    }
}

fn is_only_public_name(schema: &Value, field: &Value) -> bool {
    references(field, schema, "#/$defs/PublicNameRef", &mut BTreeSet::new())
        && !references(field, schema, "#/$defs/GameTextRef", &mut BTreeSet::new())
}

fn references<'schema>(
    node: &'schema Value,
    schema: &'schema Value,
    target: &str,
    seen: &mut BTreeSet<&'schema str>,
) -> bool {
    if let Some(reference) = node.get("$ref").and_then(Value::as_str) {
        if reference == target {
            return true;
        }
        if seen.insert(reference)
            && let Some(pointer) = reference.strip_prefix('#')
            && let Some(referred) = schema.pointer(pointer)
            && references(referred, schema, target, seen)
        {
            return true;
        }
    }
    match node {
        Value::Object(fields) => fields
            .values()
            .any(|value| references(value, schema, target, seen)),
        Value::Array(values) => values
            .iter()
            .any(|value| references(value, schema, target, seen)),
        _ => false,
    }
}

#[test]
fn manufacturer_localization_uses_separate_public_and_protected_text_domains() {
    let schema = p4kx_export::catalog_schema();
    let localization = &schema["$defs"]["ManufacturerLocalization"];
    let properties = &localization["properties"];
    assert!(is_only_public_name(&schema, &properties["name"]));
    assert!(references(
        &properties["description"],
        &schema,
        "#/$defs/GameTextRef",
        &mut BTreeSet::new()
    ));
    assert_eq!(properties["description"]["x-gameText"], true);
    assert!(properties["name"].get("x-gameText").is_none());
    for field in ["sourceType", "name", "description"] {
        assert!(
            localization["required"]
                .as_array()
                .expect("required localization fields")
                .iter()
                .any(|required| required == field)
        );
    }
}

#[test]
fn loot_inline_text_domains_mark_only_the_protected_original_text() {
    let schema = p4kx_export::catalog_schema();
    let variants = schema["$defs"]["SourceText"]["oneOf"]
        .as_array()
        .expect("the original public and protected inline text variants");
    assert_eq!(variants.len(), 2, "both text domains are explicit");
    let mut domains = BTreeSet::new();
    for variant in variants {
        let domain = variant["properties"]["type"]["const"]
            .as_str()
            .expect("one explicit domain tag");
        assert!(domains.insert(domain), "each text domain occurs once");
        assert!(matches!(domain, "public" | "gameText"), "known text domain");
        let text = &variant["properties"]["sourceText"];
        assert_eq!(text["type"], "string", "original text is present text");
        assert_eq!(
            text.get("x-gameText"),
            (domain == "gameText").then_some(&Value::Bool(true)),
            "only the protected original sourceText is annotated"
        );
        let required = variant["required"]
            .as_array()
            .expect("required inline text");
        for field in ["type", "sourceText"] {
            assert!(
                required.iter().any(|value| value == field),
                "{field} is required"
            );
        }
    }
    assert_eq!(domains, BTreeSet::from(["gameText", "public"]));
}

#[test]
fn physical_shop_inventory_separates_technical_and_protected_text() {
    let schema = p4kx_export::catalog_schema();
    let variants = schema
        .pointer("/$defs/JsonString/oneOf")
        .and_then(Value::as_array)
        .expect("three typed shop inventory string variants");
    let expected = [
        ("public", "text", false),
        ("gameText", "text", true),
        ("raw", "token", true),
    ];
    assert_eq!(
        variants.len(),
        expected.len(),
        "complete string-domain inventory"
    );
    for (kind, field, protected) in expected {
        let variant = variants
            .iter()
            .find(|value| {
                value
                    .pointer("/properties/type/const")
                    .and_then(Value::as_str)
                    == Some(kind)
            })
            .expect("each shop inventory text domain has a distinct discriminator");
        let payload = variant
            .get("properties")
            .and_then(|fields| fields.get(field))
            .expect("the original text or undecodable token is preserved");
        assert_eq!(
            payload.get("x-gameText").and_then(Value::as_bool) == Some(true),
            protected,
            "{kind}.{field} has its independently defined publication domain",
        );
    }
}

#[test]
fn shop_inventory_candidate_protects_rejected_path_text() {
    let schema = p4kx_export::catalog_schema();
    let variants = schema
        .pointer("/$defs/InventoryCandidate/oneOf")
        .and_then(Value::as_array)
        .expect("typed shop inventory candidates");
    let expected = [
        ("file", Some(false)),
        ("noDeclaredFile", None),
        ("unscanned", None),
        ("rejected", Some(true)),
    ];
    assert_eq!(
        variants.len(),
        expected.len(),
        "each declaration variant is accounted for"
    );
    for (kind, protected) in expected {
        let variant = variants
            .iter()
            .find(|value| {
                value
                    .pointer("/properties/type/const")
                    .and_then(Value::as_str)
                    == Some(kind)
            })
            .expect("each candidate has a distinct discriminator");
        let source = variant.pointer("/properties/sourcePath");
        assert_eq!(
            source.map(|field| field.get("x-gameText").and_then(Value::as_bool) == Some(true)),
            protected,
            "{kind} preserves its publication domain",
        );
    }
}
