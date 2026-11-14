// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! A synthetic Archive with ships that carry insurance data, ships that carry none and an item.

use std::error::Error;
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use p4kx_export::{BuiltCatalog, CatalogOptions, Filter, build_catalog};
use p4kx_extraction::Archive;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue,
};
use serde_json::Value;

pub(crate) const INSURED: u128 = 0x6000_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const UNINSURED: u128 = 0x6000_0000_0000_4000_8000_0000_0000_0002;
pub(crate) const BARE: u128 = 0x6000_0000_0000_4000_8000_0000_0000_0003;
pub(crate) const ITEM: u128 = 0x6000_0000_0000_4000_8000_0000_0000_0004;
pub(crate) const RECOVERY: u128 = 0x6000_0000_0000_4000_8000_0000_0000_0005;

pub(crate) const RECOVERY_PATH: &str =
    "libs/foundry/records/entitlementpolicies/globalitemrecovery";

pub(crate) fn structures(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .structure(TestStruct::new("StaticEntityClassData"))
        .structure(
            TestStruct::new("ShipInsuranceParams")
                .field("shipEntityClassName", TestType::String)
                .field("baseWaitTimeMinutes", TestType::Single)
                .field("mandatoryWaitTimeMinutes", TestType::Single)
                .field("baseExpeditingFee", TestType::UInt32),
        )
        .structure(
            TestStruct::new("SEntityInsuranceProperties")
                .parent("StaticEntityClassData")
                .field("restoreOnClaim", TestType::Bool)
                .field(
                    "shipInsuranceParams",
                    TestType::Class("ShipInsuranceParams".to_owned()),
                ),
        )
        .structure(TestStruct::new("EntityClassDefinition").array(
            "StaticEntityClassData",
            TestType::StrongPointer("StaticEntityClassData".to_owned()),
        ))
        .structure(
            TestStruct::new("ItemRecoveryEscalatingClaimsParams")
                .field("escalatingClaimChainLimit", TestType::Int32)
                .field("escalationTimerMultiplier", TestType::Single),
        )
        .structure(
            TestStruct::new("ItemRecoveryOverrideGroupDef")
                .field("claimCostMultiplier", TestType::Single)
                .array("classes", TestType::Reference),
        )
        .structure(
            TestStruct::new("ItemRecoveryEconomyParams")
                .field("claimCostMultiplier", TestType::Single)
                .field("maxExpeditedClaimCostMultiplier", TestType::Single)
                .field("expeditedDeliverySecondsPerAUEC", TestType::Single)
                .field(
                    "escalatingClaimsParams",
                    TestType::Class("ItemRecoveryEscalatingClaimsParams".to_owned()),
                )
                .array(
                    "overrides",
                    TestType::Class("ItemRecoveryOverrideGroupDef".to_owned()),
                ),
        )
        .structure(TestStruct::new("ItemRecoveryConfigurationParams").field(
            "economyParams",
            TestType::Class("ItemRecoveryEconomyParams".to_owned()),
        ))
}

pub(crate) fn override_group(multiplier: f32, classes: Vec<TestValue>) -> TestValue {
    TestValue::Struct(
        TestInstance::new("ItemRecoveryOverrideGroupDef")
            .value("claimCostMultiplier", TestValue::Single(multiplier))
            .value("classes", TestValue::Array(classes)),
    )
}

pub(crate) fn recovery_record(
    guid: u128,
    name: &str,
    path: &str,
    claim_cost: f32,
    overrides: Vec<TestValue>,
) -> TestRecord {
    TestRecord::new(
        guid,
        "ItemRecoveryConfigurationParams",
        format!("ItemRecoveryConfigurationParams.{name}"),
        format!("{path}.xml"),
    )
    .value(
        "economyParams",
        TestValue::Struct(
            TestInstance::new("ItemRecoveryEconomyParams")
                .value("claimCostMultiplier", TestValue::Single(claim_cost))
                .value("maxExpeditedClaimCostMultiplier", TestValue::Single(0.0003))
                .value("expeditedDeliverySecondsPerAUEC", TestValue::Single(5e-7))
                .value(
                    "escalatingClaimsParams",
                    TestValue::Struct(
                        TestInstance::new("ItemRecoveryEscalatingClaimsParams")
                            .value("escalatingClaimChainLimit", TestValue::Int32(1))
                            .value("escalationTimerMultiplier", TestValue::Single(1.0)),
                    ),
                )
                .value("overrides", TestValue::Array(overrides)),
        ),
    )
}

fn recovery() -> TestRecord {
    recovery_record(
        RECOVERY,
        "GlobalItemRecovery",
        RECOVERY_PATH,
        0.003,
        vec![
            override_group(
                1.0,
                vec![TestValue::Reference(INSURED), TestValue::Reference(BARE)],
            ),
            override_group(0.6, Vec::new()),
        ],
    )
}

fn params(class: &str, claim: f32, mandatory: f32, fee: u32) -> TestValue {
    TestValue::Struct(
        TestInstance::new("ShipInsuranceParams")
            .value("shipEntityClassName", TestValue::text(class))
            .value("baseWaitTimeMinutes", TestValue::Single(claim))
            .value("mandatoryWaitTimeMinutes", TestValue::Single(mandatory))
            .value("baseExpeditingFee", TestValue::UInt32(fee)),
    )
}

fn insurance(restore: bool, params: TestValue) -> TestValue {
    TestValue::Struct(
        TestInstance::new("SEntityInsuranceProperties")
            .value("restoreOnClaim", TestValue::Bool(restore))
            .value("shipInsuranceParams", params),
    )
}

pub(crate) fn entity(guid: u128, class: &str, path: &str, data: Vec<TestValue>) -> TestRecord {
    TestRecord::new(
        guid,
        "EntityClassDefinition",
        format!("EntityClassDefinition.{class}"),
        format!("libs/foundry/records/entities/{path}.xml"),
    )
    .value("StaticEntityClassData", TestValue::Array(data))
}

pub(crate) fn database() -> DataForgeBuilder {
    structures(DataForgeBuilder::new().version(8))
        .record(entity(
            INSURED,
            "AEGS_Insured",
            "spaceships/aegs_insured",
            vec![insurance(true, params("AEGS_Insured", 5.83, 1.94, 2335))],
        ))
        .record(entity(
            UNINSURED,
            "AEGS_Uninsured",
            "spaceships/aegs_uninsured",
            Vec::new(),
        ))
        .record(entity(
            BARE,
            "AEGS_Bare",
            "groundvehicles/aegs_bare",
            vec![insurance(false, params("", 0.0, 0.0, 0))],
        ))
        .record(entity(
            ITEM,
            "item_with_insurance",
            "scitem/misc/item_with_insurance",
            vec![insurance(true, params("Item", 1.0, 1.0, 1))],
        ))
        .record(recovery())
}

/// Writes the fixture Archive into `dir` and returns its path.
pub(crate) fn write_archive(dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    write_forge(dir, &database())
}

fn write_forge(dir: &Path, forge: &DataForgeBuilder) -> Result<PathBuf, Box<dyn Error>> {
    let path = dir.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", forge.build()).encrypted())
        .write_to(&path)?;
    Ok(path)
}

/// Builds the catalog of an Archive that holds `forge`, under `filter`, and parses its JSON.
pub(crate) fn build_from(
    forge: &DataForgeBuilder,
    filter: Option<&Filter>,
) -> Result<(BuiltCatalog, Value), Box<dyn Error>> {
    build_mode(forge, filter, p4kx_export::CatalogPublicationMode::Normal)
}

/// Builds an intentionally unreadable or unsupported recovery fixture for diagnosis.
pub(crate) fn diagnostic_from(
    forge: &DataForgeBuilder,
    filter: Option<&Filter>,
) -> Result<(BuiltCatalog, Value), Box<dyn Error>> {
    build_mode(
        forge,
        filter,
        p4kx_export::CatalogPublicationMode::DiagnosticPartial,
    )
}

fn build_mode(
    forge: &DataForgeBuilder,
    filter: Option<&Filter>,
    publication_mode: p4kx_export::CatalogPublicationMode,
) -> Result<(BuiltCatalog, Value), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive = Archive::open_with_key(
        write_forge(dir.path(), forge)?,
        Some(p4kx_testkit::test_key()),
    )?;
    let options = CatalogOptions {
        publication_mode,
        filter,
        ..CatalogOptions::new(UNIX_EPOCH + Duration::from_secs(1_792_000_000))
    };
    let built = build_catalog(&archive, &options)?;
    let catalog = serde_json::from_slice(built.json())?;
    Ok((built, catalog))
}
