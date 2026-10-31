// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::{Kind, is_kind_leftover, is_leftover};

const LOOT_TABLES: &str = "libs/foundry/records/lootgeneration/loottables/x";

#[test]
fn the_v3_and_the_older_loot_structs_of_one_folder_are_different_kinds() {
    assert_eq!(
        Kind::by_record(LOOT_TABLES, "LootTableV3Record"),
        Some(Kind::LootTables)
    );
    assert_eq!(
        Kind::by_record(LOOT_TABLES, "LootTable"),
        Some(Kind::LegacyLootTables)
    );
    let archetypes = "libs/foundry/records/lootgeneration/lootarchetypes/x";
    assert_eq!(
        Kind::by_record(archetypes, "LootArchetypeV3Record"),
        Some(Kind::LootArchetypes)
    );
    assert_eq!(
        Kind::by_record(archetypes, "LootArchetype"),
        Some(Kind::LegacyLootArchetypes)
    );
}

#[test]
fn a_wrong_struct_under_a_prefix_or_a_path_outside_every_prefix_is_no_kind() {
    assert_eq!(Kind::by_record(LOOT_TABLES, "Tag"), None);
    assert_eq!(
        Kind::by_record("libs/foundry/records/other/x", "LootTable"),
        None
    );
}

#[test]
fn every_kind_but_the_older_loot_kinds_and_the_crafters_is_found_alike_by_path_and_by_record() {
    for kind in Kind::ALL {
        let Some(rule) = kind.rule().filter(|_| kind != Kind::Crafters) else {
            continue;
        };
        for prefix in rule.prefixes {
            let path = format!("{prefix}x");
            let by_record = Kind::by_record(&path, rule.type_name);
            assert_eq!(by_record, Some(kind), "{kind} by record at {path}");
            let by_path = Kind::by_path(&path);
            match kind {
                Kind::LegacyLootTables => assert_eq!(by_path, Some(Kind::LootTables)),
                Kind::LegacyLootArchetypes => assert_eq!(by_path, Some(Kind::LootArchetypes)),
                _ => assert_eq!(by_path, by_record, "{kind} by path at {path}"),
            }
        }
    }
}

#[test]
fn the_leftover_like_ship_names_are_excluded_as_whole_words_only() {
    let ship = |path: &str, name: &str| is_kind_leftover(Kind::Ships, path, name);
    for path in [
        "libs/foundry/records/entities/spaceships/glsn_shiv_temp_cosa",
        "libs/foundry/records/entities/spaceships/aegs_reclaimer_bis2024_temp",
        "libs/foundry/records/entities/spaceships/aegs_avenger_stalker_display",
        "libs/foundry/records/entities/spaceships/orig_890jump_hijacked_pisces",
        "libs/foundry/records/entities/spaceships/drak_caterpillar_pu_hijacked",
    ] {
        assert!(ship(path, "Ship"), "{path}");
    }
    assert!(ship(
        "libs/foundry/records/x",
        "RSI_Meteor_TEMP_LaserRepeater"
    ));
    for path in [
        "libs/foundry/records/entities/spaceships/rsi_temperature_probe",
        "libs/foundry/records/entities/scitem/displays/panel",
        "libs/foundry/records/entities/spaceships/contemporary",
        "libs/foundry/records/entities/spaceships/drak_hijacker",
    ] {
        assert!(!ship(path, "Ship"), "{path}");
    }
    assert!(
        !is_kind_leftover(
            Kind::Items,
            "libs/foundry/records/entities/scitem/ships/displays/radar_display_1m",
            "radar_display_1m"
        ),
        "an item keeps a display word"
    );
}

#[test]
fn a_token_is_a_whole_word_of_the_path_or_the_name() {
    for path in [
        "libs/foundry/records/entities/scitem/weapons/fps_weapons/dev/vlk_spewgun",
        "libs/foundry/records/entities/scitem/usables/counter_bar_1m_deprecated",
        "libs/foundry/records/entities/scitem/carryables/1h/chip_01_debug",
    ] {
        assert!(is_leftover(path, "Name"), "{path}");
    }
    assert!(is_leftover("libs/foundry/records/x", "Chip_DEBUG"), "name");
    for path in [
        "libs/foundry/records/entities/scitem/devices/scanner",
        "libs/foundry/records/entities/scitem/fps_devices/behr_ltp_01",
        "libs/foundry/records/entities/scitem/undeveloped",
        "libs/foundry/records/entities/spaceships/devastator",
    ] {
        assert!(!is_leftover(path, "Scanner"), "{path}");
    }
    assert!(
        !is_leftover("libs/foundry/records/entities/spaceships/x", "Devastator"),
        "a name that starts with the token"
    );
}

#[test]
fn a_contract_template_is_no_leftover_for_the_word_template_but_for_the_other_tokens() {
    let template = |path: &str, name: &str| is_kind_leftover(Kind::ContractTemplates, path, name);
    let folder = "libs/foundry/records/contracts/contracttemplates/";
    assert!(!template(
        &format!("{folder}handyman_template"),
        "ContractTemplate.Handyman_Template"
    ));
    assert!(template(
        &format!("{folder}dev_temp"),
        "ContractTemplate.Dev_Temp"
    ));
    assert!(template(&format!("{folder}x"), "ContractTemplate.Debug"));
    assert!(is_leftover(&format!("{folder}handyman_template"), "x"));
}
