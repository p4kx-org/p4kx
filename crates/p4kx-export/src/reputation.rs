// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{RecordTree, StructValue};

use crate::contract_reputation::{FactionReputation, RelationStanding};
use crate::guid_text::CatalogGuid;
use crate::mission_layers::{key, one};
use crate::record_fields::{boolean, class_name, reference, references, structs, text};
use crate::texts::Texts;

/// The name of the `propertiesBB` entry of a faction that states whether it is lawful.
const LAWFUL_PROPERTY: &str = "entityLawful";
/// The struct of the `dynamicProperty` of a `propertiesBB` entry that holds a boolean `value`.
const BOOL_PROPERTY: &str = "SBBDynamicPropertyBool";

/// Returns the faction the Record tree `tree` describes, its name resolved by `texts`.
pub(crate) fn faction_reputation<'f>(
    tree: &RecordTree<'f>,
    texts: &Texts<'f>,
) -> FactionReputation<'f> {
    let root = Some(tree.root());
    FactionReputation {
        guid: CatalogGuid(tree.guid()),
        class_name: class_name(tree),
        record_path: tree.record_path(),
        display_name: texts.localised(key(root, "displayName")),
        is_npc: boolean(root, "isNPC"),
        lawful: lawful(root),
        context_guid: reference(root, "reputationContextPropertiesUI"),
        ally_guids: references(root, "allies"),
        enemy_guids: references(root, "enemies"),
        hostility: relation(root, "hostilityParams"),
        allied: relation(root, "alliedParams"),
    }
}

/// Returns the value of the boolean `entityLawful` property among the `propertiesBB` of `root`.
fn lawful(root: Option<&StructValue<'_>>) -> Option<bool> {
    structs(root, "propertiesBB")
        .find(|property| text(Some(property), "name") == Some(LAWFUL_PROPERTY))
        .and_then(|property| one(Some(property), "dynamicProperty"))
        .filter(|value| value.type_name() == BOOL_PROPERTY)
        .and_then(|value| boolean(Some(value), "value"))
}

/// Returns the scope and standing the `RelationStandingParams` in the field `name` of `root`
/// names, `None` when it names neither.
fn relation(root: Option<&StructValue<'_>>, name: &str) -> Option<RelationStanding> {
    let params = one(root, name);
    let scope_guid = reference(params, "scope");
    let standing_guid = reference(params, "standing");
    (scope_guid.is_some() || standing_guid.is_some()).then_some(RelationStanding {
        scope_guid,
        standing_guid,
    })
}
