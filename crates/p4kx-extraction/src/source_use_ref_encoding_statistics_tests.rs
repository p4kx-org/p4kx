// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use crate::{
    InputScope, SourceEvidenceError, SourceInput, SourceInventory, SourceLocation,
    SourceUseDescription, SourceUseRole, SourceUses,
};

#[test]
fn statistics_coordinate_references_preserve_distinct_locations() -> Result<(), Box<dyn Error>> {
    let locations = [
        site("dataforgePropertyDefinition", "instanceLayout", &[64]),
        site(
            "dataforgePropertyDefinition",
            "targetedInstanceLayout",
            &[64],
        ),
        site("dataforgePropertyDefinition", "name", &[64]),
        site("dataforgeStructDefinition", "firstProperty", &[64]),
        site("dataforgeStructDefinition", "parent", &[64]),
        site("dataforgeStructDefinition", "propertyCount", &[64]),
        site("dataforgeStructDefinition", "size", &[64]),
        site("dataforgeStructDefinition", "sourceType", &[64]),
        site("dataforgeDecodedBytes", "arrayHeader", &[0, 64]),
        site("dataforgeDecodedBytes", "pointer", &[0, 64]),
        site("dataforgeDecodedBytes", "textTerminator", &[0, 64]),
        site("dataforgeDecodedBytes", "text", &[0, 64]),
        site("dataforgeDecodedBytes", "text", &[64, 128]),
        site("dataforgeDecodedBytes", "slot", &[0, 64]),
        site("dataforgeDecodedBytes", "text", &[u64::MAX - 1, u64::MAX]),
        site("dataforgeInstance", "selection", &[0, 64]),
    ];
    let standard = capture(&SourceUses::with_compact_coordinates(), &locations)?;
    let statistics = capture(&SourceUses::with_field_statistics_coordinates(), &locations)?;
    let reversed = capture(
        &SourceUses::with_field_statistics_coordinates(),
        locations.iter().rev(),
    )?;
    assert_eq!(statistics.use_count(), locations.len(), "no sites collide");
    assert_eq!(
        serde_json::to_vec(&statistics)?,
        serde_json::to_vec(&reversed)?,
        "source uses are independent of registration order"
    );
    for location in &locations {
        assert!(
            find(&statistics, location)?.id().as_str().len()
                < find(&standard, location)?.id().as_str().len(),
            "only the identifier for {location:?} is shorter"
        );
    }
    Ok(())
}

#[test]
fn statistics_uses_the_original_encoding_for_unrecognized_sites() -> Result<(), Box<dyn Error>> {
    let locations = [
        site("dataforgePropertyDefinition", "unknown", &[64]),
        site("dataforgePropertyDefinition", "name", &[0, 64]),
        site("dataforgeDecodedBytes", "text", &[64]),
        site("other", "selection", &[0, 64]),
    ];
    let standard = capture(&SourceUses::with_compact_coordinates(), &locations)?;
    let statistics = capture(&SourceUses::with_field_statistics_coordinates(), &locations)?;
    assert_eq!(
        serde_json::to_vec(&statistics)?,
        serde_json::to_vec(&standard)?,
        "only the recognized physical coordinate shapes shorten"
    );
    Ok(())
}

fn capture<'location>(
    uses: &SourceUses,
    locations: impl IntoIterator<Item = &'location SourceLocation>,
) -> Result<SourceInventory, SourceEvidenceError> {
    let scope = uses.observe(SourceInput::Unknown, InputScope::Payload)?;
    for location in locations {
        scope.at(location.clone(), SourceUseRole::Selector)?;
    }
    uses.finish_with(|| Ok(()))
}

fn find<'inventory>(
    inventory: &'inventory SourceInventory,
    location: &SourceLocation,
) -> Result<&'inventory SourceUseDescription, Box<dyn Error>> {
    inventory
        .iter_uses()
        .find(|used| used.location() == location)
        .ok_or_else(|| "the original typed coordinates remain in the inventory".into())
}

fn site(namespace: &str, field: &str, indices: &[u64]) -> SourceLocation {
    SourceLocation::Subresource {
        namespace: namespace.to_owned(),
        field: field.to_owned(),
        indices: indices.to_vec(),
    }
}
