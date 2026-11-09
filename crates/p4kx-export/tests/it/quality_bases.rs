// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Fieldless quality sources through the public crafting and catalog interfaces.

use std::error::Error;

use p4kx_export::CatalogError;
use p4kx_extraction::DataForge;
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::catalog_crafting_support;
use crate::quality_v3_support as source;

type TestResult = Result<(), Box<dyn Error>>;

const BASE: &str = "CraftingQualityDistribution_Base";
const NON_REF: &str = "CraftingQualityDistribution_Base_NonRef";

#[derive(Clone, Copy)]
enum BaseShape {
    Empty,
    AdditionalField,
}

fn database(shape: BaseShape) -> DataForgeBuilder {
    let base = match shape {
        BaseShape::Empty => TestStruct::new(BASE),
        BaseShape::AdditionalField => {
            TestStruct::new(BASE).field("additionalField", TestType::String)
        }
    };
    let builder = DataForgeBuilder::new()
        .version(8)
        .structure(base)
        .structure(TestStruct::new(NON_REF).parent(BASE))
        .structure(TestStruct::new("CraftingQualityDistributionRecord").field(
            "qualityDistribution",
            TestType::StrongPointer(BASE.to_owned()),
        ))
        .structure(TestStruct::new("ResourceTypeCraftingData").field(
            "qualityDistribution",
            TestType::StrongPointer(BASE.to_owned()),
        ))
        .structure(TestStruct::new("ResourceType").array(
            "properties",
            TestType::Class("ResourceTypeCraftingData".to_owned()),
        ));
    add_records(builder)
}

fn add_records(mut builder: DataForgeBuilder) -> DataForgeBuilder {
    for (name, quality_guid, resource_guid) in [(BASE, 0x8101, 0x8201), (NON_REF, 0x8102, 0x8202)] {
        builder = builder.record(
            TestRecord::new(
                quality_guid,
                "CraftingQualityDistributionRecord",
                format!("CraftingQualityDistributionRecord.{name}"),
                format!("libs/foundry/records/crafting/qualitydistribution/{name}.xml"),
            )
            .value(
                "qualityDistribution",
                TestValue::Struct(TestInstance::new(name)),
            ),
        );
        builder = builder.record(
            TestRecord::new(
                resource_guid,
                "ResourceType",
                format!("ResourceType.{name}"),
                format!("libs/foundry/records/resourcetypedatabase/{name}.xml"),
            )
            .value(
                "properties",
                TestValue::Array(vec![TestValue::Struct(
                    TestInstance::new("ResourceTypeCraftingData").value(
                        "qualityDistribution",
                        TestValue::Struct(TestInstance::new(name)),
                    ),
                )]),
            ),
        );
    }
    builder
}

#[test]
fn public_crafting_keeps_both_known_fieldless_curve_kinds() -> TestResult {
    let bytes = database(BaseShape::Empty).build();
    let forge = DataForge::open(&bytes)?;
    let crafting = p4kx_export::read_crafting(&forge, None, None)?;
    assert_eq!(crafting.failures, []);
    assert_eq!(crafting.quality_distributions.len(), 2);
    for (record, name) in crafting.quality_distributions.iter().zip([BASE, NON_REF]) {
        let curve = record
            .distribution
            .as_ref()
            .ok_or("the fieldless source curve")?;
        assert_eq!(curve.kind, name);
        assert_eq!(
            (curve.min, curve.max, curve.mean, curve.stddev),
            (None, None, None, None)
        );
    }
    Ok(())
}

#[test]
fn catalog_preserves_fieldless_top_level_and_resource_source_forms() -> TestResult {
    let (_, catalog) = catalog_crafting_support::build_from(&database(BaseShape::Empty), None)?;
    for (index, name) in [BASE, NON_REF].into_iter().enumerate() {
        assert_eq!(
            source::at(
                source::rows(&catalog, source::DISTRIBUTIONS)?
                    .get(index)
                    .ok_or("quality row")?,
                "/qualityDistribution/value/sourceType/value"
            )?,
            &json!(name)
        );
        assert_eq!(
            source::at(
                source::rows(&catalog, source::DISTRIBUTIONS)?
                    .get(index)
                    .ok_or("quality row")?,
                "/qualityDistribution/value/payload/value/type"
            )?,
            &json!("emptyBase")
        );
        assert_eq!(
            catalog["resourceTypes"][index]["properties"][0]["payload"]["qualityDistribution"],
            json!({
                "sourceType": name, "payload": {"type": "emptyBase"},
            })
        );
    }
    assert_eq!(catalog["exportHealth"]["status"], "complete");
    Ok(())
}

#[test]
fn an_additional_declared_base_field_requires_diagnostic_publication() -> TestResult {
    let database = database(BaseShape::AdditionalField);
    let result = catalog_crafting_support::build_from(&database, None);
    let Err(error) = result else {
        return Err("an unmapped base field must refuse normal publication".into());
    };
    assert!(matches!(
        error.downcast_ref::<CatalogError>(),
        Some(CatalogError::IncompleteExport { .. })
    ));
    let (_, catalog) = catalog_crafting_support::diagnostic_from(&database, None)?;
    assert_eq!(
        Some(source::rows(&catalog, source::DISTRIBUTIONS)?.len()),
        Some(2)
    );
    assert_eq!(catalog["resourceTypes"].as_array().map(Vec::len), Some(2));
    let diagnostics = catalog["exportHealth"]["diagnostics"]
        .as_array()
        .ok_or("the diagnostics")?;
    let has_field = diagnostics.iter().any(|diagnostic| {
        diagnostic["code"] == "unsupportedValue"
            && diagnostic["examples"].as_array().is_some_and(|examples| {
                examples
                    .iter()
                    .any(|example| example["field"] == "qualityDistribution.additionalField")
            })
    });
    assert!(
        has_field,
        "a fieldless base cannot silently discard an added source field"
    );
    assert_ne!(
        catalog["exportHealth"]["status"],
        Value::String("complete".to_owned())
    );
    Ok(())
}
