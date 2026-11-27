// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestType, TestValue};

use super::{builder, component};

pub(crate) const BASE_GUID: u128 = 0x2000_0000_0000_4000_8000_0000_0000_0101;
pub(crate) const OTHER_PLAYER_RECORD_INDEX: u32 = 0;
pub(crate) const BASE_RECORD_INDEX: u32 = 1;
pub(crate) const EQUAL_RANK_RECORD_INDEX: u32 = 2;
pub(crate) const SELECTED_RECORD_INDEX: u32 = 3;
pub(crate) const WRONG_KEY_RECORD_INDEX: u32 = 4;
pub(crate) const SELECTED_FILTER: &str = "fixture_variant_selected";

const SELECTED_GUID: u128 = BASE_GUID + 1;
const OTHER_PLAYER_GUID: u128 = BASE_GUID + 2;
const ITEMS_ROOT: &str = "libs/foundry/records/entities/scitem";
const PURCHASABLE: &str = "SCItemPurchasableParams";
const ENTITY: &str = "EntityClassDefinition";

pub(crate) fn database() -> DataForgeBuilder {
    builder()
        .structure(component(PURCHASABLE).field("displayName", TestType::Locale))
        .record(player(
            OTHER_PLAYER_GUID,
            "M_Player_Alternative",
            "other_player",
            1,
        ))
        .record(player(
            BASE_GUID,
            "EntityClassDefinition.K_Player_Base",
            "base",
            1,
        ))
        .record(player(BASE_GUID, "K_Player_Base", "equal_rank_alias", 1))
        .record(record(
            SELECTED_GUID,
            "A_Mission_Item",
            &format!("mission_entities/{SELECTED_FILTER}"),
            1,
            "@LOC_PLACEHOLDER",
        ))
        .record(player(BASE_GUID, "B_WrongKey", "wrong_key", 2))
}

fn player(guid: u128, name: &str, filename: &str, grade: i32) -> TestRecord {
    record(
        guid,
        name,
        &format!("fixture/{filename}"),
        grade,
        "@later_name",
    )
}

fn record(guid: u128, name: &str, path: &str, grade: i32, display_name: &str) -> TestRecord {
    let attachment = TestInstance::new("SAttachableComponentParams").value(
        "AttachDef",
        TestValue::Struct(
            TestInstance::new("SItemDefinition")
                .value("Type", TestValue::text("PowerPlant"))
                .value("Size", TestValue::Int32(1))
                .value("Grade", TestValue::Int32(grade))
                .value(
                    "Localization",
                    TestValue::Struct(
                        TestInstance::new("SCItemLocalization")
                            .value("Name", TestValue::text("@later_name")),
                    ),
                ),
        ),
    );
    let purchase =
        TestInstance::new(PURCHASABLE).value("displayName", TestValue::text(display_name));
    TestRecord::new(guid, ENTITY, name, format!("{ITEMS_ROOT}/{path}.xml")).value(
        "Components",
        TestValue::Array(vec![
            TestValue::Struct(attachment),
            TestValue::Struct(purchase),
        ]),
    )
}
