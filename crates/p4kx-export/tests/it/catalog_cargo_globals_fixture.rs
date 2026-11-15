// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(super) const LOADING_TYPE: &str = "GlobalCargoLoadingParams";
pub(super) const SHOP_TYPE: &str = "GlobalShopCommodityParams";
pub(super) const TIME_TYPE: &str = "SAutoLoadingBoxSizeLoadingTime";
pub(super) const PRICE_TYPE: &str = "SAutoLoadingBoxSizePrices";
pub(super) const SHARED_LOADING_GUID: u128 = 0x1152;
pub(super) const LOADING_FILTER: &str = "synthetic/cargo/loading-b";

const TIME_MEMBERS: [(&str, f32); 9] = [
    ("thirtyTwoSCU", 32.25),
    ("one_eighthSCU", -0.0),
    ("sixteenSCU", 16.25),
    ("one_halfSCU", 0.5),
    ("fourSCU", 4.25),
    ("twentyFourSCU", 24.25),
    ("twoSCU", 2.25),
    ("eightSCU", 8.25),
    ("oneSCU", f32::from_bits(0x7fc0_1152)),
];

const PRICE_MEMBERS: [(&str, i32); 10] = [
    ("oneSCU", 0),
    ("one_quarterSCU", 250),
    ("one_halfSCU", -500),
    ("one_eighthSCU", i32::MIN),
    ("twoSCU", 2000),
    ("fourSCU", 4000),
    ("eightSCU", 8000),
    ("sixteenSCU", 16000),
    ("twentyFourSCU", 24000),
    ("thirtyTwoSCU", i32::MAX),
];

#[derive(Clone, Copy)]
pub(super) enum Layout {
    Authored,
    DuplicateIdentity,
    InvalidTimeMember,
}

pub(super) fn database(layout: Layout) -> DataForgeBuilder {
    let mut times = TestStruct::new(TIME_TYPE).field("futureTimeFlag", TestType::Bool);
    for (member, _) in TIME_MEMBERS {
        let storage = if invalid_member(layout, member) {
            TestType::UInt32
        } else {
            TestType::Single
        };
        times = times.field(member, storage);
    }
    let mut prices = TestStruct::new(PRICE_TYPE);
    for (member, _) in PRICE_MEMBERS {
        prices = prices.field(member, TestType::Int32);
    }
    let second_guid = if matches!(layout, Layout::DuplicateIdentity) {
        SHARED_LOADING_GUID
    } else {
        0x1154
    };
    DataForgeBuilder::new()
        .version(8)
        .structure(times)
        .structure(prices)
        .structure(
            TestStruct::new(LOADING_TYPE)
                .field("autoLoadingBaseUnloadingTime", TestType::Single)
                .field("autoLoadingBoxSizeUnloadingTime", class(TIME_TYPE))
                .field("cargoDeckLoadingTimePerSCU", TestType::Single)
                .field("autoLoadingBaseLoadingTime", TestType::Single)
                .field("autoLoadingBoxSizeLoadingTime", class(TIME_TYPE))
                .field("futureCargoFlag", TestType::Bool),
        )
        .structure(
            TestStruct::new(SHOP_TYPE)
                .field("autoLoadingBoxSizePrices", class(PRICE_TYPE))
                .field("autoLoadingBasePrice", TestType::Int32),
        )
        .record(loading(SHARED_LOADING_GUID, "loading-a", 1.0, layout))
        .record(loading(second_guid, "loading-b", 2.0, layout))
        .record(shop())
}

fn loading(guid: u128, name: &str, base: f32, layout: Layout) -> TestRecord {
    TestRecord::new(
        guid,
        LOADING_TYPE,
        format!("{LOADING_TYPE}.{name}"),
        format!("synthetic/cargo/{name}.xml"),
    )
    .value("autoLoadingBaseLoadingTime", TestValue::Single(base))
    .value("autoLoadingBaseUnloadingTime", TestValue::Single(0.0))
    .value("cargoDeckLoadingTimePerSCU", TestValue::Single(0.25))
    .value("autoLoadingBoxSizeLoadingTime", times(layout, 1.0))
    .value("autoLoadingBoxSizeUnloadingTime", times(layout, -1.0))
    .value("futureCargoFlag", TestValue::Bool(false))
}

fn times(layout: Layout, direction: f32) -> TestValue {
    let mut instance = TestInstance::new(TIME_TYPE).value("futureTimeFlag", TestValue::Bool(true));
    for (member, duration) in TIME_MEMBERS {
        let value = if invalid_member(layout, member) {
            TestValue::UInt32(42)
        } else {
            TestValue::Single(duration * direction)
        };
        instance = instance.value(member, value);
    }
    TestValue::Struct(instance)
}

fn shop() -> TestRecord {
    let mut prices = TestInstance::new(PRICE_TYPE);
    for (member, price) in PRICE_MEMBERS {
        prices = prices.value(member, TestValue::Int32(price));
    }
    TestRecord::new(
        0x1153,
        SHOP_TYPE,
        "GlobalShopCommodityParams.prices",
        "synthetic/cargo/shop.xml",
    )
    .value("autoLoadingBasePrice", TestValue::Int32(-300))
    .value("autoLoadingBoxSizePrices", TestValue::Struct(prices))
}

fn invalid_member(layout: Layout, name: &str) -> bool {
    matches!(layout, Layout::InvalidTimeMember) && name == "one_halfSCU"
}

fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}
