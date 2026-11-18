// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogPublicationMode, build_catalog};
use p4kx_extraction::Archive;
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::Value;

use crate::commodity_v3_fixture::{self as source, DcbOrigin, TestResult};

const ENTITY: &str = "EntityClassDefinition";
const BASE: &str = "DataForgeComponentParams";
const ATTACHMENT: &str = "SAttachableComponentParams";
const DEFINITION: &str = "SItemDefinition";
const MAKER: &str = "SCItemManufacturer";

pub(super) struct Fixture {
    pub(super) archive: Archive,
    pub(super) catalog: Value,
    _folder: tempfile::TempDir,
}

pub(super) fn read(origin: DcbOrigin, cross_family: bool) -> TestResult<Fixture> {
    let (folder, archive) = source::open(&database(cross_family), origin)?;
    let mut options = source::options();
    if matches!(origin, DcbOrigin::Unknown) {
        options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    }
    let catalog = serde_json::from_slice(build_catalog(&archive, &options)?.json())?;
    Ok(Fixture {
        archive,
        catalog,
        _folder: folder,
    })
}

fn database(cross_family: bool) -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new(ENTITY).array("Components", TestType::StrongPointer(BASE.into())),
        )
        .structure(TestStruct::new(BASE))
        .structure(
            TestStruct::new(ATTACHMENT)
                .parent(BASE)
                .field("AttachDef", TestType::Class(DEFINITION.into())),
        )
        .structure(TestStruct::new(DEFINITION).field("Type", TestType::String))
        .structure(TestStruct::new(MAKER).field("Code", TestType::String))
        .record(item("Alpha"))
        .record(item("Beta"))
        .record(
            TestRecord::new(
                if cross_family { 1 } else { 2 },
                MAKER,
                "SCItemManufacturer.Physical",
                "libs/foundry/records/scitemmanufacturer/fixture/physical.xml",
            )
            .value("Code", TestValue::text("Fixture")),
        )
}

fn item(name: &str) -> TestRecord {
    TestRecord::new(
        1,
        ENTITY,
        format!("{ENTITY}.{name}"),
        format!("libs/foundry/records/entities/scitem/fixture/physical_{name}.xml"),
    )
    .value(
        "Components",
        TestValue::Array(vec![attachment("First"), attachment("Later")]),
    )
}

fn attachment(label: &str) -> TestValue {
    let definition = TestInstance::new(DEFINITION).value("Type", TestValue::text(label));
    TestValue::Struct(
        TestInstance::new(ATTACHMENT).value("AttachDef", TestValue::Struct(definition)),
    )
}
