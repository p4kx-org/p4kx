// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use super::inventory::{Disposition, FieldKey, Inventory, Mapping};
use super::path::{Path, Step};

fn component(populated_empty: bool, missing: bool) -> TestInstance {
    TestInstance::new("SyntheticParams")
        .value(
            "variant",
            TestValue::Struct(
                TestInstance::new("SyntheticConcrete").value("value", TestValue::Single(7.0)),
            ),
        )
        .value(
            "silent",
            TestValue::Struct(
                TestInstance::new("SyntheticNested").value("amount", TestValue::Single(2.0)),
            ),
        )
        .value(
            "weights",
            TestValue::Array(vec![TestValue::Single(0.0), TestValue::Single(3.0)]),
        )
        .value(
            "future",
            if populated_empty {
                TestValue::Struct(
                    TestInstance::new("SyntheticFuture").value("amount", TestValue::Single(5.0)),
                )
            } else {
                TestValue::Null
            },
        )
        .value(
            "missing",
            if missing {
                TestValue::Reference(0x1111_0000_0000_4000_8000_0000_0000_0004)
            } else {
                TestValue::Null
            },
        )
}

fn schema(base: &str) -> DataForgeBuilder {
    let structures = vec![
        TestStruct::new("EntityClassDefinition").array(
            "Components",
            TestType::StrongPointer("DataForgeComponentParams".to_owned()),
        ),
        TestStruct::new("DataForgeComponentParams"),
        TestStruct::new("SyntheticBase"),
        TestStruct::new("SyntheticNewBase"),
        TestStruct::new("SyntheticConcrete")
            .parent(base)
            .field("value", TestType::Single),
        TestStruct::new("SyntheticNested").field("amount", TestType::Single),
        TestStruct::new("SyntheticFuture").field("amount", TestType::Single),
        TestStruct::new("SyntheticParams")
            .parent("DataForgeComponentParams")
            .field("variant", TestType::StrongPointer(base.to_owned()))
            .field("silent", TestType::Class("SyntheticNested".to_owned()))
            .array("weights", TestType::Single)
            .field(
                "future",
                TestType::StrongPointer("SyntheticFuture".to_owned()),
            )
            .field("missing", TestType::Reference),
    ];
    structures
        .into_iter()
        .fold(DataForgeBuilder::new(), DataForgeBuilder::structure)
}

fn database(base: &str, populated_empty: bool, missing: bool) -> Vec<u8> {
    schema(base)
        .record(
            TestRecord::new(
                0x1111_0000_0000_4000_8000_0000_0000_0001,
                "EntityClassDefinition",
                "EntityClassDefinition.SyntheticCoverage",
                "libs/foundry/records/entities/scitem/synthetic_coverage.xml",
            )
            .value(
                "Components",
                TestValue::Array(vec![TestValue::Struct(component(populated_empty, missing))]),
            ),
        )
        .build()
}

pub(crate) fn probe_database(changed_target: bool, populated_empty: bool) -> Vec<u8> {
    database(
        if changed_target {
            "SyntheticNewBase"
        } else {
            "SyntheticBase"
        },
        populated_empty,
        true,
    )
}

pub(crate) fn healthy_probe_database() -> Vec<u8> {
    database("SyntheticBase", false, false)
}

fn mapped(owner: &str, field: &str, target: &str, empty: bool) -> Mapping {
    Mapping {
        source: FieldKey::new(owner, field),
        declared_target: None,
        disposition: Disposition::Exported {
            group: "items".to_owned(),
            path: Path::fields(&["stats", "probe", target]),
        },
        previously_empty: empty,
    }
}

fn structural() -> Vec<Mapping> {
    vec![
        Mapping {
            source: FieldKey::new("SyntheticParams", "variant"),
            declared_target: Some("SyntheticBase".to_owned()),
            disposition: Disposition::ReferenceOnly {
                reason: "owned container; concrete fields are mapped separately".to_owned(),
            },
            previously_empty: false,
        },
        Mapping {
            source: FieldKey::new("SyntheticParams", "silent"),
            declared_target: Some("SyntheticNested".to_owned()),
            disposition: Disposition::Exported {
                group: "items".to_owned(),
                path: Path::fields(&["stats", "probe"]),
            },
            previously_empty: false,
        },
        Mapping {
            source: FieldKey::new("SyntheticParams", "future"),
            declared_target: Some("SyntheticFuture".to_owned()),
            disposition: Disposition::Unsupported {
                reason:
                    "schema-declared resource override needs a domain projection when populated"
                        .to_owned(),
            },
            previously_empty: true,
        },
        Mapping {
            source: FieldKey::new("SyntheticParams", "missing"),
            declared_target: None,
            disposition: Disposition::ReferenceOnly {
                reason: "stored Record link; missing targets are counted separately".to_owned(),
            },
            previously_empty: false,
        },
    ]
}

pub(crate) fn probe_inventory() -> Inventory {
    let mut fields = structural();
    fields.extend([
        mapped("SyntheticConcrete", "value", "value", false),
        mapped("SyntheticNested", "amount", "amount", false),
        mapped("SyntheticFuture", "amount", "future", true),
    ]);
    for array_element in [false, true] {
        let mut mapping = mapped("SyntheticParams", "weights", "weights", false);
        mapping.source.array_element = array_element;
        if let Disposition::Exported { path, .. } = &mut mapping.disposition
            && array_element
        {
            path.0.push(Step::Each);
        }
        fields.push(mapping);
    }
    Inventory {
        schema_version: 1,
        roots: vec!["SyntheticParams".to_owned()],
        fields,
    }
}
