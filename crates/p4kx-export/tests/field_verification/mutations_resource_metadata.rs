// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::{self, Component, Path, PathBuf};

use p4kx_extraction::DataForge;
use p4kx_testkit::{DataForgeBuilder, TestStruct};
use serde_json::Value;

use super::{resource_declarations, resource_metadata_fixture, resource_metadata_output};

#[test]
fn declarations_include_owned_types_without_surveying_values_links_or_enum_options()
-> super::Result<()> {
    let bytes = resource_metadata_fixture::database();
    let forge = DataForge::open(&bytes)?;
    let declarations = resource_declarations::collect(&forge, &|| false)?;
    let json = serde_json::to_string(&declarations)?;
    for forbidden in [
        "SENTINEL",
        "273.125",
        "doNotTraverseReference",
        "doNotTraverseWeak",
        "doNotTraverseSibling",
    ] {
        assert!(
            !json.contains(forbidden),
            "only declared resource-owned schema metadata may be serialized: {forbidden}"
        );
    }
    let value: Value = serde_json::from_str(&json)?;
    let types = value["types"].as_array().ok_or("schema types absent")?;
    for expected in [
        "ResourceType",
        "ResourceTypeGroup",
        "DeclarationParent",
        "ResourceTypeProperties",
        "ConcreteResourceProperty",
        "OwnedMetadata",
        "PropertyDetails",
    ] {
        assert!(
            types.iter().any(|owner| owner["name"] == expected),
            "owned or inherited schema declaration missing: {expected}"
        );
    }
    let fields = types
        .iter()
        .find(|owner| owner["name"] == "ResourceType")
        .and_then(|owner| owner["fields"].as_array())
        .ok_or("resource fields absent")?;
    assert!(
        fields.iter().any(|field| field["name"] == "properties"
            && field["array"] == true
            && field["targetType"] == "ResourceTypeProperties"),
        "array/target declaration metadata is retained"
    );
    assert!(
        fields.iter().any(
            |field| field["name"] == "link" && field["targetType"] == "ExternalReferenceTarget"
        ),
        "Reference target type is named without following it"
    );
    assert!(
        fields
            .iter()
            .any(|field| field["name"] == "unit" && field["targetType"] == "ResourceUnit"),
        "enum type name is retained without option contents"
    );
    Ok(())
}

#[test]
fn metadata_collection_preserves_cancellation_and_rejects_an_empty_schema_scope()
-> super::Result<()> {
    let bytes = resource_metadata_fixture::database();
    let forge = DataForge::open(&bytes)?;
    assert!(
        resource_declarations::collect(&forge, &|| true).is_err(),
        "metadata-only traversal remains cancellable"
    );
    let empty = DataForgeBuilder::new()
        .structure(TestStruct::new("Unrelated"))
        .build();
    assert!(
        resource_declarations::collect(&DataForge::open(&empty)?, &|| false).is_err(),
        "no requested root cannot provide resource declaration evidence"
    );
    Ok(())
}

#[test]
fn metadata_artifacts_require_containment_and_never_overwrite_existing_bytes() -> super::Result<()>
{
    let directory = tempfile::tempdir()?;
    let root = std::fs::canonicalize(directory.path())?;
    let output = root.join("resource-schema.json");
    let destination = resource_metadata_output::Destination::new(&root, &output)?;
    destination.write(b"{}")?;
    assert!(
        destination.write(b"changed").is_err(),
        "a metadata artifact must be a new explicitly selected file"
    );
    assert_eq!(
        std::fs::read(&output)?,
        b"{}",
        "existing artifact bytes remain intact"
    );
    let outside = root
        .parent()
        .ok_or("task parent absent")?
        .join("outside-resource-schema.json");
    assert!(
        resource_metadata_output::Destination::new(&root, &outside).is_err(),
        "metadata output cannot escape its task root"
    );
    let ambiguous = raw_parent_path(&root);

    assert!(
        ambiguous
            .components()
            .any(|part| part == Component::ParentDir),
        "the mutation retains an actual parent component instead of normalized input"
    );
    assert!(
        resource_metadata_output::Destination::new(&root, &ambiguous).is_err(),
        "ambiguous output paths cannot bypass containment"
    );
    assert!(
        resource_metadata_output::Destination::new(&root, &root.join("wrong.bin")).is_err(),
        "metadata output has an explicit JSON type"
    );
    let oversized =
        resource_metadata_output::Destination::new(&root, &root.join("oversized.json"))?;
    assert!(
        oversized.write(&vec![0; (8 << 20) + 1]).is_err(),
        "an oversized artifact fails before creating a partial file"
    );
    assert!(
        !root.join("oversized.json").exists(),
        "failed bounded output creates no artifact"
    );
    Ok(())
}

#[test]
fn unknown_owned_targets_and_duplicate_declarations_do_not_claim_a_complete_inventory()
-> super::Result<()> {
    let invalid = DataForgeBuilder::new()
        .structure(TestStruct::new("ResourceType").field(
            "owned",
            p4kx_testkit::TestType::Class("MissingOwnedType".to_owned()),
        ))
        .build();
    assert!(
        resource_declarations::collect(&DataForge::open(&invalid)?, &|| false).is_err(),
        "an unknown owned target cannot be silently omitted"
    );
    let duplicate = DataForgeBuilder::new()
        .structure(TestStruct::new("ResourceType"))
        .structure(TestStruct::new("ResourceType"))
        .build();
    assert!(
        resource_declarations::collect(&DataForge::open(&duplicate)?, &|| false).is_err(),
        "duplicate schema identities cannot overwrite one another"
    );
    Ok(())
}

fn raw_parent_path(root: &Path) -> PathBuf {
    let mut raw = root.as_os_str().to_os_string();
    for component in ["nested", "..", "escape.json"] {
        raw.push(path::MAIN_SEPARATOR_STR);
        raw.push(component);
    }
    PathBuf::from(raw)
}
