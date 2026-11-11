// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, StructValue, Value};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestStruct, TestType, TestValue};

use crate::commodity_v3_fixture::TestResult;
use crate::quality_v3_fixture_schema as schema;
use crate::quality_v3_fixture_values as values;

#[derive(Clone, Copy)]
pub(super) enum Case {
    Healthy,
    Width,
    Reversed,
    Missing,
    Unknown,
}

pub(super) fn database(case: Case) -> DataForgeBuilder {
    match case {
        Case::Healthy => schema::schema().record(values::distribution(
            values::DISTRIBUTION,
            "Healthy",
            values::normal(
                TestValue::Int32(0),
                TestValue::Double(7.5),
                TestValue::Double(1.25),
            ),
        )),
        Case::Width => schema::numeric_schema(TestType::UInt64, TestType::Double, TestType::Double)
            .record(values::distribution(
                values::DISTRIBUTION,
                "WrongWidth",
                values::normal(
                    TestValue::UInt64(u64::MAX),
                    TestValue::Double(7.5),
                    TestValue::Double(1.25),
                ),
            )),
        Case::Reversed => schema::schema().record(values::quantization(
            values::QUANTIZATION,
            "Reversed",
            "reversed",
            values::bands(vec![
                values::band(0, 9, 3),
                values::band(50, 40, 4),
                values::band(0, 9, 3),
            ]),
        )),
        Case::Missing => missing(),
        Case::Unknown => crate::resource_reference_fixture::unknown_property(),
    }
}

fn missing() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("CraftingQualityQuantization_Base"))
        .structure(TestStruct::new("CraftingQualityQuantizationRecord").field(
            "qualityQuantization",
            TestType::StrongPointer("CraftingQualityQuantization_Base".to_owned()),
        ))
        .structure(
            TestStruct::new("CraftingQualityQuantization")
                .parent("CraftingQualityQuantization_Base")
                .array(
                    "bands",
                    TestType::Class("CraftingQualityQuantizationBand".to_owned()),
                ),
        )
        .structure(
            TestStruct::new("CraftingQualityQuantizationBand")
                .field("end", TestType::Int32)
                .field("mappedValue", TestType::Int32),
        )
        .record(values::quantization(
            values::QUANTIZATION,
            "AbsentStart",
            "absent_start",
            values::bands(vec![TestValue::Struct(
                TestInstance::new("CraftingQualityQuantizationBand")
                    .value("end", TestValue::Int32(9))
                    .value("mappedValue", TestValue::Int32(3)),
            )]),
        ))
}

fn structure<'tree, 'source>(
    value: Option<&'tree Value<'source>>,
) -> TestResult<&'tree StructValue<'source>> {
    match value {
        Some(Value::Struct(source)) => Ok(source),
        _ => Err("the required actual original Struct".into()),
    }
}

pub(super) fn original(database: &DataForgeBuilder, case: Case) -> TestResult {
    let bytes = database.build();
    let forge = DataForge::open(&bytes)?;
    let record = forge
        .record(0)
        .ok_or("the actual original physical Record")?;
    let tree = record.tree()?;
    assert!(
        tree.is_from(&record),
        "the source tree is an actual immutable read of physical Record0"
    );
    assert_eq!(
        record.index(),
        0,
        "the independently selected physical position"
    );
    assert_eq!(
        tree.error_count(),
        0,
        "the source is fully readable; guard failures cannot be blamed on decode loss"
    );
    match case {
        Case::Healthy | Case::Width => original_distribution(&record, &tree, case)?,
        Case::Reversed | Case::Missing => original_bands(&tree, case)?,
        Case::Unknown => {
            let Some(Value::Array(properties)) = tree.field("properties") else {
                return Err("actual original property slots".into());
            };
            let property = structure(properties.first())?;
            assert_eq!(
                property.type_name(),
                "FixtureFutureProperty",
                "the unsupported concrete type is genuinely encoded"
            );
            assert!(
                matches!(property.field("name"), Some(Value::String("future"))),
                "the actual readable neighbor is retained"
            );
            assert!(
                matches!(property.field("opaque"), Some(Value::Int32(42))),
                "the actual unknown source field exists before projection"
            );
        }
    }
    Ok(())
}

fn original_distribution(
    record: &p4kx_extraction::Record<'_>,
    tree: &p4kx_extraction::RecordTree<'_>,
    case: Case,
) -> TestResult {
    assert_eq!(
        record.type_name()?,
        "CraftingQualityDistributionRecord",
        "actual original family selection"
    );
    let curve = structure(tree.field("qualityDistribution"))?;
    assert_eq!(
        curve.type_name(),
        "CraftingQualityDistributionNormal",
        "the actual concrete original curve"
    );
    match case {
        Case::Healthy => assert!(
            matches!(curve.field("min"), Some(Value::Int32(0))),
            "the independent valid counterpart has actual Int32 storage"
        ),
        Case::Width => assert!(
            matches!(curve.field("min"), Some(Value::UInt64(u64::MAX))),
            "the original wrong width and all64 source bits exist before either producer"
        ),
        _ => unreachable!("the outer match restricts these two source cases"),
    }
    Ok(())
}

fn original_bands(tree: &p4kx_extraction::RecordTree<'_>, case: Case) -> TestResult {
    let quantization = structure(tree.field("qualityQuantization"))?;
    let Some(Value::Array(bands)) = quantization.field("bands") else {
        return Err("actual authored bands".into());
    };
    if matches!(case, Case::Reversed) {
        assert_eq!(
            bands.len(),
            3,
            "repeated original slots preserve their actual positions"
        );
        let middle = structure(bands.get(1))?;
        assert!(
            matches!(middle.field("start"), Some(Value::Int32(50))),
            "actual original start"
        );
        assert!(
            matches!(middle.field("end"), Some(Value::Int32(40))),
            "actual original reversed end"
        );
    } else {
        assert_eq!(
            bands.len(),
            1,
            "one original band with a genuinely missing declaration"
        );
        let band = structure(bands.first())?;
        assert!(
            band.field("start").is_none(),
            "the original declaration is absent, not a null or fake Value"
        );
        let names: Vec<_> = band
            .fields()
            .iter()
            .map(p4kx_extraction::Field::name)
            .collect();
        assert_eq!(
            names,
            ["end", "mappedValue"],
            "the entire original field-name survey independently proves absence"
        );
    }
    Ok(())
}
