// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::Kind;
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

const POOL: &str = "BlueprintPoolRecord";
const REWARD: &str = "BlueprintReward";
const MAKER: &str = "SCItemManufacturer";
const QUALITY: &str = "CraftingQualityQuantizationRecord";
pub(super) const GUID: u128 = 0x9e71;
const DISTINCT_MAKER: u128 = 0x9e72;
const DISTINCT_QUALITY: u128 = 0x9e73;

#[derive(Debug, Clone, Copy)]
pub(super) enum Collision {
    None,
    Manufacturer,
    Quality,
}

impl Collision {
    pub(super) const fn other(self) -> Option<Kind> {
        match self {
            Self::None => None,
            Self::Manufacturer => Some(Kind::Manufacturers),
            Self::Quality => Some(Kind::QualityQuantizations),
        }
    }
}

pub(super) fn database(collision: Collision) -> DataForgeBuilder {
    crate::commodity_v3_fixture_schema::database()
        .structure(TestStruct::new("PoolIdentityRewardBase"))
        .structure(
            TestStruct::new(REWARD)
                .parent("PoolIdentityRewardBase")
                .field("blueprintRecord", TestType::Reference)
                .field("weight", TestType::Single),
        )
        .structure(TestStruct::new(POOL).array(
            "blueprintRewards",
            TestType::StrongPointer("PoolIdentityRewardBase".to_owned()),
        ))
        .structure(TestStruct::new(MAKER).field("Code", TestType::String))
        .record(manufacturer(
            if matches!(collision, Collision::Manufacturer) {
                GUID
            } else {
                DISTINCT_MAKER
            },
        ))
        .record(quality(if matches!(collision, Collision::Quality) {
            GUID
        } else {
            DISTINCT_QUALITY
        }))
        .record(pool("First"))
        .record(pool("Second"))
}

fn pool(name: &str) -> TestRecord {
    TestRecord::new(
        GUID,
        POOL,
        format!("{POOL}.{name}"),
        "libs/foundry/records/crafting/blueprintrewards/identity.xml",
    )
    .value("blueprintRewards", TestValue::Array(Vec::new()))
}

fn manufacturer(guid: u128) -> TestRecord {
    TestRecord::new(
        guid,
        MAKER,
        format!("{MAKER}.PoolIdentity"),
        "libs/foundry/records/scitemmanufacturer/fixture/pool_identity.xml",
    )
    .value("Code", TestValue::text("Fixture"))
}

fn quality(guid: u128) -> TestRecord {
    let band = TestInstance::new("CraftingQualityQuantizationBand")
        .value("start", TestValue::Int32(0))
        .value("end", TestValue::Int32(100))
        .value("mappedValue", TestValue::Int32(50));
    let quantization = TestInstance::new("CraftingQualityQuantization")
        .value("bands", TestValue::Array(vec![TestValue::Struct(band)]));
    TestRecord::new(
        guid,
        QUALITY,
        format!("{QUALITY}.PoolIdentity"),
        "libs/foundry/records/crafting/qualityquantization/pool_identity.xml",
    )
    .value("qualityQuantization", TestValue::Struct(quantization))
}
