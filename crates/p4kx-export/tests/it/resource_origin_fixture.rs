// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::path::Path;
use std::time::{Duration, UNIX_EPOCH};

use p4kx_export::CatalogOptions;
use p4kx_extraction::{Archive, Language};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue,
};

pub(crate) type TestResult<T = ()> = Result<T, Box<dyn Error>>;

pub(crate) fn options() -> CatalogOptions<'static> {
    CatalogOptions::new(UNIX_EPOCH + Duration::from_secs(1_792_000_000))
}

pub(crate) fn database(number: f32) -> DataForgeBuilder {
    database_with_texts(number, "@fixture_name", "@fixture_description")
}

pub(crate) fn database_with_texts(number: f32, name: &str, description: &str) -> DataForgeBuilder {
    database_schema()
        .record(resource_record(number, name, description))
        .record(
            TestRecord::new(
                2,
                "ResourceTypeGroup",
                "ResourceTypeGroup.Fixture",
                "libs/foundry/records/resourcetypedatabase/group.xml",
            )
            .value(
                "resources",
                TestValue::Array(vec![TestValue::Reference(1), TestValue::Reference(1)]),
            ),
        )
        .record(TestRecord::new(
            3,
            "EntityClassDefinition",
            "EntityClassDefinition.Fixture",
            "libs/foundry/records/entities/fixture/container.xml",
        ))
}

fn resource_type_schema() -> TestStruct {
    TestStruct::new("ResourceType")
        .field("displayName", TestType::Locale)
        .field("description", TestType::Locale)
        .field(
            "densityType",
            TestType::StrongPointer("ResourceTypeDensityType".to_owned()),
        )
        .field("defaultThumbnailPath", TestType::String)
        .field("validateDefaultCargoBox", TestType::Bool)
        .field(
            "refinedVersion",
            TestType::ReferenceTo("ResourceType".to_owned()),
        )
        .field(
            "rttThumbnailEntityClass",
            TestType::ReferenceTo("EntityClassDefinition".to_owned()),
        )
        .field(
            "defaultCargoContainers",
            TestType::StrongPointer("SResourceTypeDefaultCargoContainers".to_owned()),
        )
        .array(
            "properties",
            TestType::StrongPointer("ResourceTypeProperties".to_owned()),
        )
}

fn database_schema() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(resource_type_schema())
        .structure(TestStruct::new("ResourceTypeDensityType"))
        .structure(
            TestStruct::new("ResourceTypeMolarMass")
                .parent("ResourceTypeDensityType")
                .field("molarMass", TestType::Single),
        )
        .structure(TestStruct::new("ResourceTypeProperties").field("name", TestType::String))
        .structure(
            TestStruct::new("ResourceTypeVolatility")
                .parent("ResourceTypeProperties")
                .field("volatility", TestType::Single)
                .field("healthDecayPerSecond", TestType::Single),
        )
        .structure(
            TestStruct::new("SResourceTypeDefaultCargoContainers")
                .field(
                    "oneSCU",
                    TestType::ReferenceTo("EntityClassDefinition".to_owned()),
                )
                .field(
                    "twoSCU",
                    TestType::ReferenceTo("EntityClassDefinition".to_owned()),
                ),
        )
        .structure(
            TestStruct::new("ResourceTypeGroup")
                .array(
                    "resources",
                    TestType::ReferenceTo("ResourceType".to_owned()),
                )
                .array(
                    "groups",
                    TestType::ReferenceTo("ResourceTypeGroup".to_owned()),
                ),
        )
        .structure(TestStruct::new("EntityClassDefinition"))
}

fn resource_record(number: f32, name: &str, description: &str) -> TestRecord {
    TestRecord::new(
        1,
        "ResourceType",
        "ResourceType.Fixture",
        "libs/foundry/records/resourcetypedatabase/fixture.xml",
    )
    .value("displayName", TestValue::text(name))
    .value("description", TestValue::text(description))
    .value("defaultThumbnailPath", TestValue::text(""))
    .value("rttThumbnailEntityClass", TestValue::Reference(3))
    .value(
        "densityType",
        TestValue::Struct(
            TestInstance::new("ResourceTypeMolarMass")
                .value("molarMass", TestValue::Single(number)),
        ),
    )
    .value(
        "defaultCargoContainers",
        TestValue::Struct(
            TestInstance::new("SResourceTypeDefaultCargoContainers")
                .value("twoSCU", TestValue::Reference(3)),
        ),
    )
    .value(
        "properties",
        TestValue::Array(vec![
            volatility("Repeated", 0.0),
            TestValue::Null,
            volatility("Repeated", 1.0),
        ]),
    )
}

fn volatility(name: &str, value: f32) -> TestValue {
    TestValue::Struct(
        TestInstance::new("ResourceTypeVolatility")
            .value("name", TestValue::text(name))
            .value("volatility", TestValue::Single(value))
            .value("healthDecayPerSecond", TestValue::Single(0.0)),
    )
}

pub(crate) fn unported() -> DataForgeBuilder {
    database(2.0)
        .structure(TestStruct::new("CraftingQualityDistribution_Base"))
        .structure(
            TestStruct::new("CraftingQualityDistributionNormal")
                .parent("CraftingQualityDistribution_Base")
                .field("min", TestType::Int32)
                .field("max", TestType::Int32)
                .field("mean", TestType::Single)
                .field("stddev", TestType::Single),
        )
        .structure(TestStruct::new("CraftingQualityDistributionRecord").field(
            "qualityDistribution",
            TestType::StrongPointer("CraftingQualityDistribution_Base".to_owned()),
        ))
        .record(
            TestRecord::new(
                4,
                "CraftingQualityDistributionRecord",
                "CraftingQualityDistributionRecord.Unported",
                "libs/foundry/records/crafting/qualitydistribution/unported.xml",
            )
            .value(
                "qualityDistribution",
                TestValue::Struct(
                    TestInstance::new("CraftingQualityDistributionNormal")
                        .value("min", TestValue::Int32(0))
                        .value("max", TestValue::Int32(100))
                        .value("mean", TestValue::Single(f32::NAN))
                        .value("stddev", TestValue::Single(1.0)),
                ),
            ),
        )
}

pub(crate) fn open(
    data: Vec<u8>,
    every_language: bool,
) -> TestResult<(tempfile::TempDir, Archive)> {
    let directory = tempfile::tempdir()?;
    let archive = write(directory.path(), data, every_language)?;
    Ok((directory, archive))
}

pub(crate) fn write(folder: &Path, data: Vec<u8>, every_language: bool) -> TestResult<Archive> {
    let path = folder.join("fixture.p4k");
    let mut builder =
        ArchiveBuilder::new().entry(p4kx_testkit::TestEntry::stored("Data\\Game2.dcb", data));
    for language in Language::ALL {
        if !every_language
            && !matches!(
                language,
                Language::English | Language::German | Language::French
            )
        {
            continue;
        }
        let text = if language == Language::English {
            "fixture_name=discarded line\nfixture_name=English winner\nfixture_description=Protected description"
        } else {
            "fixture_name=Translated name\nfixture_description=Translated description"
        };
        let entry = p4kx_testkit::TestEntry::stored(language.global_ini_path(), text.as_bytes());
        let entry = match language {
            Language::English => entry.encrypted(),
            Language::French => entry.encrypted().with_encryption_field(7),
            _ => entry,
        };
        builder = builder.entry(entry);
    }
    builder
        .entry(p4kx_testkit::TestEntry::stored(
            "DATA/LOCALIZATION/ENGLISH/GLOBAL.INI",
            b"fixture_name=discarded duplicate Entry",
        ))
        .entry(p4kx_testkit::TestEntry::stored(
            "Data\\Libs\\CosmoTools\\LayerRecovery\\LayerBackups.xml",
            b"<LayerBackups/>",
        ))
        .write_to(&path)?;
    Ok(Archive::open_with_key(
        path,
        Some(p4kx_testkit::test_key()),
    )?)
}
