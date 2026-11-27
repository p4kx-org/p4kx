// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::CraftingWarning;

use super::WarningJson;

#[test]
fn the_kind_of_every_warning_in_the_json_is_the_kind_the_library_names() {
    let warnings = [
        CraftingWarning::UnreadableRewardRecords(1),
        CraftingWarning::UnknownModifierRanges {
            count: 1,
            details: vec!["c".to_owned()],
        },
        CraftingWarning::AmbiguousDismantle(2),
        CraftingWarning::UnreadableVehicleFiles {
            count: 1,
            details: vec!["a".to_owned()],
        },
        CraftingWarning::UnplacedStarmapContainers(3),
        CraftingWarning::UnreadableShopRegistry,
        CraftingWarning::MissingShopRegistry,
        CraftingWarning::UnreadableShopContainers(4),
        CraftingWarning::UnreadableShopInventories {
            count: 1,
            details: vec!["b".to_owned()],
        },
        CraftingWarning::MissingShopInventories(5),
        CraftingWarning::NoShopComponents,
    ];
    for warning in &warnings {
        let json = serde_json::to_value(WarningJson::from(warning));
        assert_eq!(
            json.as_ref().ok().and_then(|json| json["kind"].as_str()),
            Some(warning.kind()),
            "{warning:?}"
        );
    }
}
