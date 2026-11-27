// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use serde_json::{Value, json};

use super::{REPEATED_TILES, guid};

pub(super) fn verify_links(rows: &[Value]) -> Result<(), Box<dyn Error>> {
    assert_eq!(
        rows.len(),
        2 * REPEATED_TILES,
        "both original slots occur at each route"
    );
    let [first, malformed, repeated, ..] = rows else {
        return Err("expected two unresolved links at each route".into());
    };
    for row in rows {
        assert_eq!(
            row["targetStatus"]["value"], "notFound",
            "exact authored target absent"
        );
        assert_eq!(
            row["targetCandidates"]["value"],
            json!([]),
            "no admitted candidate"
        );
        assert_eq!(
            row["declaration"]["declaredPath"]["value"],
            "Data/objectcontainers/pu/system/t/ghost.socpak",
            "authored path"
        );
    }
    verify_source_identity(first, malformed, repeated);

    assert_eq!(
        first["declaration"]["localPosition"]["value"],
        json!([2.0, 0.0, 0.0])
    );
    assert_eq!(
        first["declaration"]["starmapRecordGuid"]["value"],
        json!(guid(3).to_string())
    );
    assert_eq!(
        malformed["declaration"]["localPosition"]["value"],
        json!([3.0, 0.0, 0.0])
    );
    assert_eq!(
        malformed["declaration"]["localRotation"]["state"],
        "invalid"
    );
    assert_eq!(malformed["worldPosition"]["state"], "invalid");
    assert_eq!(first["worldPosition"]["value"], json!([3.0, 0.0, 0.0]));
    let attributes = malformed["declaration"]["attributes"]
        .as_array()
        .ok_or("original attributes")?;
    assert!(
        attributes.iter().any(|attribute| {
            attribute["name"]["value"] == "rot"
                && attribute["value"]["value"] == "broken authored quaternion"
        }),
        "malformed original transform retains its exact text"
    );
    Ok(())
}

fn verify_source_identity(first: &Value, malformed: &Value, repeated: &Value) {
    assert_eq!(
        first["declaration"], repeated["declaration"],
        "both placements share the one physical declaration"
    );
    assert_ne!(
        first["instanceId"], repeated["instanceId"],
        "same declaration at distinct routes"
    );
    assert_ne!(
        first["declaration"]["declaredPath"], malformed["declaration"]["declaredPath"],
        "equal path text from separate physical Child sites"
    );
}

pub(super) fn verify_parents(
    containers: &[Value],
    unresolved: &[Value],
) -> Result<(), Box<dyn Error>> {
    let keys: Vec<_> = containers
        .iter()
        .map(|container| &container["instanceId"]["value"])
        .collect();
    for row in unresolved {
        let parent = &row["parentInstanceId"]["value"];
        assert!(
            keys.contains(&parent),
            "actual unresolved parent exists in container rows"
        );
        let id = row["instanceId"]["value"]["childNodeIndices"]
            .as_array()
            .ok_or("unresolved instance route")?;
        let prefix = parent["childNodeIndices"]
            .as_array()
            .ok_or("parent route")?;
        assert!(id.starts_with(prefix), "actual parent Child-route prefix");
        let descriptor = &row["declaration"]["descriptor"]["value"];
        assert_eq!(
            descriptor["entryIndex"],
            json!(0),
            "physical main XML in tile"
        );
        let declaring = &row["declaringContainer"]["value"];
        let parent_row = containers
            .iter()
            .find(|container| &container["instanceId"]["value"] == parent)
            .ok_or("original parent row")?;
        assert_eq!(
            declaring, &parent_row["container"]["value"],
            "actual declaring container identity"
        );
        assert_ne!(
            descriptor["inputRef"], declaring["inputRef"],
            "nested descriptor source kept distinct from its outer container"
        );
    }
    Ok(())
}
