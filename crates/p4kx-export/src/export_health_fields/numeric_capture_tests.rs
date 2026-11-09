// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::time::UNIX_EPOCH;

use p4kx_extraction::{Archive, DataForge, Filter, TreeBuilder};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue,
};
use serde_json::Value;

use crate::export_diagnostic::ExportSourceRole;
use crate::{CatalogError, CatalogOptions, CatalogPublicationMode, ExportDiagnosticCode, Kind};

type TestResult = Result<(), Box<dyn Error>>;

fn database(alias: bool) -> DataForgeBuilder {
    let builder = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("FixtureComponent"))
        .structure(TestStruct::new("EntityClassDefinition").array(
            "Components",
            TestType::StrongPointer("FixtureComponent".to_owned()),
        ))
        .structure(
            TestStruct::new("ResourceContainer")
                .parent("FixtureComponent")
                .array(
                    "defaultComposition",
                    TestType::Class("FixtureCompositionEntry".to_owned()),
                ),
        )
        .structure(
            TestStruct::new("FixtureCompositionEntry")
                .field("entry", TestType::ReferenceTo("ResourceType".to_owned())),
        )
        .structure(TestStruct::new("ResourceType").field(
            "densityType",
            TestType::StrongPointer("ResourceTypeDensityType".to_owned()),
        ))
        .structure(TestStruct::new("ResourceTypeDensityType"))
        .structure(
            TestStruct::new("ResourceTypeDensity")
                .parent("ResourceTypeDensityType")
                .field(
                    "densityUnit",
                    TestType::StrongPointer("BaseDensityUnit".to_owned()),
                ),
        )
        .structure(TestStruct::new("BaseDensityUnit"))
        .structure(
            TestStruct::new("GramsPerCubicCentimeter")
                .parent("BaseDensityUnit")
                .field("gramsPerCubicCentimeter", TestType::Single),
        )
        .record(resource(
            "Original",
            "excluded",
            f32::from_bits(0x7fc0_1234),
        ))
        .record(commodity());
    if alias {
        builder.record(resource("Alias", "included", 2.5))
    } else {
        builder
    }
}

fn commodity() -> TestRecord {
    TestRecord::new(
        2,
        "EntityClassDefinition",
        "EntityClassDefinition.FixtureCommodity",
        "libs/foundry/records/entities/commodities/included/fixture.xml",
    )
    .value(
        "Components",
        TestValue::Array(vec![TestValue::Struct(
            TestInstance::new("ResourceContainer").value(
                "defaultComposition",
                TestValue::Array(vec![TestValue::Struct(
                    TestInstance::new("FixtureCompositionEntry")
                        .value("entry", TestValue::Reference(1)),
                )]),
            ),
        )]),
    )
}

fn resource(name: &str, directory: &str, number: f32) -> TestRecord {
    TestRecord::new(
        1,
        "ResourceType",
        format!("ResourceType.{name}"),
        format!("libs/foundry/records/resourcetypedatabase/{directory}/{name}.xml"),
    )
    .value(
        "densityType",
        TestValue::Struct(
            TestInstance::new("ResourceTypeDensity").value(
                "densityUnit",
                TestValue::Struct(
                    TestInstance::new("GramsPerCubicCentimeter")
                        .value("gramsPerCubicCentimeter", TestValue::Single(number)),
                ),
            ),
        ),
    )
}

fn open(builder: &DataForgeBuilder) -> Result<(tempfile::TempDir, Archive), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("numeric-capture.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\Game2.dcb", builder.build()))
        .entry(TestEntry::stored(
            "Data\\Libs\\CosmoTools\\LayerRecovery\\LayerBackups.xml",
            b"<LayerBackups/>",
        ))
        .write_to(&path)?;
    Ok((directory, Archive::open(path)?))
}

#[test]
fn repeated_resource_inspections_are_identical_across_projection_contexts() -> TestResult {
    let bytes = database(false).build();
    let forge = DataForge::open(&bytes)?;
    let record = forge
        .records()
        .find(|record| record.type_name().ok() == Some("ResourceType"))
        .ok_or("ResourceType fixture missing")?;
    let mut trees = TreeBuilder::new();
    let tree = trees.build(&record)?;
    let mut selected = crate::export_field_walk::inspect(
        &tree,
        &record,
        Some(Kind::ResourceTypes),
        ExportSourceRole::Selected,
        None,
    )?;
    let mut dependency = crate::export_field_walk::inspect(
        &tree,
        &record,
        Some(Kind::QualityQuantizations),
        ExportSourceRole::Dependency,
        None,
    )?;
    let mut selected = selected
        .field_inspections
        .pop()
        .ok_or("selected proof missing")?;
    let mut dependency = dependency
        .field_inspections
        .pop()
        .ok_or("dependency proof missing")?;
    selected.contextualize(None, ExportSourceRole::Dependency);
    dependency.contextualize(None, ExportSourceRole::Dependency);
    assert_eq!(
        selected, dependency,
        "canonical evidence does not depend on the requesting projection"
    );
    assert_eq!(
        selected.number_requirements.len(),
        1,
        "the raw number still has conditional failure evidence"
    );
    Ok(())
}

#[test]
fn emitted_resource_bits_cover_the_same_physical_commodity_dependency() -> TestResult {
    let (_directory, archive) = open(&database(false))?;
    let built = crate::build_catalog(&archive, &CatalogOptions::new(UNIX_EPOCH))?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    assert_eq!(catalog["exportHealth"]["status"], "complete");
    assert_eq!(
        catalog["definitions"]["resourceTypes"]["rows"][0]["density"]["value"]["payload"]["value"]
            ["densityUnit"]["value"]["payload"]["value"]["value"]["value"]["rawBits"],
        "7fc01234"
    );
    assert_eq!(
        catalog["definitions"]["commodities"]["rows"]
            .as_array()
            .map(Vec::len),
        Some(1),
        "the physical Commodity definition is actually projected"
    );
    assert_eq!(
        catalog["projections"]["commodityPresentation"]["rows"]
            .as_array()
            .map(Vec::len),
        Some(1),
        "the same dependency has its canonical Commodity presentation"
    );
    assert!(catalog.get("commodities").is_none());
    Ok(())
}

#[test]
fn filtered_resource_bits_cannot_cover_an_unexported_commodity_dependency() -> TestResult {
    let (_directory, archive) = open(&database(false))?;
    let filter = Filter::new("included");
    let options = CatalogOptions {
        filter: Some(&filter),
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    assert!(
        matches!(
            crate::build_catalog(&archive, &options),
            Err(CatalogError::IncompleteExport { .. })
        ),
        "an unexported raw value cannot excuse the finite-only Commodity presentation"
    );
    assert_missing_capture(&archive, options)
}

#[test]
fn a_same_guid_resource_alias_does_not_cover_the_unexported_physical_source() -> TestResult {
    let (_directory, archive) = open(&database(true))?;
    let filter = Filter::new("included");
    let options = CatalogOptions {
        filter: Some(&filter),
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    assert!(
        matches!(
            crate::build_catalog(&archive, &options),
            Err(CatalogError::IncompleteExport { .. })
        ),
        "a nominal GUID alias does not prove the physical source's raw value was emitted"
    );
    assert_missing_capture(&archive, options)
}

fn assert_missing_capture(archive: &Archive, mut options: CatalogOptions<'_>) -> TestResult {
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = crate::build_catalog(archive, &options)?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    assert_eq!(catalog["exportHealth"]["status"], "incomplete");
    assert_eq!(
        catalog["definitions"]["commodities"]["rows"]
            .as_array()
            .map(Vec::len),
        Some(1),
        "the physical Commodity definition survives diagnostic publication"
    );
    assert_eq!(
        catalog["projections"]["commodityPresentation"]["rows"]
            .as_array()
            .map(Vec::len),
        Some(1),
        "the presentation survives with its nullable derived density"
    );
    assert!(catalog.get("commodities").is_none());
    let density = catalog
        .pointer("/projections/commodityPresentation/rows/0/densityGramsPerCubicCentimetre")
        .ok_or("canonical Commodity density carrier missing")?;
    assert_eq!(density["state"], "present");
    assert!(
        density.get("value").is_some_and(Value::is_null),
        "the unrepresentable derived density has an explicit nullable value"
    );
    assert!(
        catalog["exportHealth"]["diagnostics"]
            .as_array()
            .ok_or("diagnostics missing")?
            .iter()
            .any(|diagnostic| diagnostic["code"] == "unsupportedValue"
                && diagnostic["stage"] == "projection"
                && diagnostic["examples"]
                    .as_array()
                    .is_some_and(|examples| examples.iter().any(|example| example["field"]
                        == "densityType.densityUnit.gramsPerCubicCentimeter"))),
        "the exact nonfinite source field remains an explicit blocking failure"
    );
    Ok(())
}

#[test]
fn unsupported_numeric_storage_keeps_the_finite_projection_failure() -> TestResult {
    let fields = [
        (
            TestStruct::new("GramsPerCubicCentimeter")
                .field("gramsPerCubicCentimeter", TestType::Double),
            TestValue::Double(f64::NAN),
        ),
        (
            TestStruct::new("GramsPerCubicCentimeter")
                .array("gramsPerCubicCentimeter", TestType::Single),
            TestValue::Array(vec![TestValue::Single(f32::NAN)]),
        ),
        (
            TestStruct::new("FixtureMolarMass").field("molarMass", TestType::Single),
            TestValue::Single(f32::NAN),
        ),
    ];
    for (index, (owner, value)) in fields.into_iter().enumerate() {
        let (name, field) = if index == 2 {
            ("FixtureMolarMass", "molarMass")
        } else {
            ("GramsPerCubicCentimeter", "gramsPerCubicCentimeter")
        };
        let bytes = unsupported_storage_database(owner, name, field, value);
        let forge = DataForge::open(&bytes)?;
        let record = forge.records().next().ok_or("resource missing")?;
        let mut trees = TreeBuilder::new();
        let tree = trees.build(&record)?;
        let mut ledger = crate::export_field_walk::inspect(
            &tree,
            &record,
            Some(Kind::ResourceTypes),
            ExportSourceRole::Selected,
            None,
        )?;
        let proof = ledger
            .field_inspections
            .pop()
            .ok_or("field proof missing")?;
        assert!(
            proof.number_requirements.is_empty(),
            "unsupported numeric shape {index} is never a candidate"
        );
        assert!(
            proof
                .diagnostics
                .iter()
                .any(|note| note.code == ExportDiagnosticCode::UnsupportedValue),
            "unsupported numeric shape {index} keeps its blocking diagnostic"
        );
    }
    Ok(())
}

fn unsupported_storage_database(
    owner: TestStruct,
    name: &str,
    field: &str,
    value: TestValue,
) -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("ResourceType")
                .field("densityType", TestType::StrongPointer(name.to_owned())),
        )
        .structure(owner)
        .record(
            TestRecord::new(
                1,
                "ResourceType",
                "ResourceType.Fixture",
                "libs/foundry/records/resourcetypedatabase/fixture.xml",
            )
            .value(
                "densityType",
                TestValue::Struct(TestInstance::new(name).value(field, value)),
            ),
        )
        .build()
}
