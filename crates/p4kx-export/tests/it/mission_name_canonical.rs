// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use super::Position;
use crate::loot_v3_graph as raw;
use crate::mission_canonical_text as canonical;
use crate::mission_template_graph as source;
use crate::mission_values_fixture::{Owner, TestResult};

pub(super) fn original(catalog: &Value, position: Position, owner: Owner) -> TestResult {
    let property = owner.property(catalog, "Case")?;
    property.kind("investigation")?;
    let name = match position {
        Position::Suspect => {
            let suspect = property.element(property.field(property.node, "suspects")?, 0)?;
            property.field(suspect, "name")?
        }
        Position::Corpse => {
            let location = property.element(property.field(property.node, "locations")?, 0)?;
            let item = property.element(property.field(location, "clueItems")?, 0)?;
            let clue = property.element(property.field(item, "clues")?, 0)?;
            property.field(clue, "name")?
        }
    };
    raw::structure(name, "MissionPropertyValue_AIName")?;
    for field in ["randomName", "randomLastName", "randomNickName"] {
        assert_eq!(
            source::scalar(property.field(name, field)?, "boolean")?,
            &json!(false),
            "the original authored name flag remains false"
        );
    }
    canonical::single(property.field(name, "chanceOfNickName")?, 0.0)?;
    source::reference(
        property.field(name, "characterNameData")?,
        "79000000-0000-4000-8000-000000000002",
        None,
    )?;
    for (field, original, meaning) in [
        ("characterGivenName", "plain", "notKey"),
        ("characterGivenLastName", "", "empty"),
        (
            "characterGivenNickName",
            "@LOC_UNINITIALIZED",
            "placeholder",
        ),
    ] {
        canonical::key(
            &property,
            catalog,
            property.field(name, field)?,
            original,
            meaning,
            None,
        )?;
    }
    Ok(())
}
