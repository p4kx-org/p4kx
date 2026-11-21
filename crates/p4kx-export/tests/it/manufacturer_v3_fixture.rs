// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{BuiltCatalog, CatalogPublicationMode, Filter, build_catalog};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::Value;

use crate::commodity_v3_fixture::{self as archive_fixture, DcbOrigin};
pub(crate) use crate::commodity_v3_fixture::{TestResult, at, guid, text_row};

pub(crate) const MAKER: u128 = 0x20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Slot {
    Code,
    Localization,
    Name,
    Description,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum State {
    Absent,
    Null,
    Empty,
    Present,
    Invalid,
    Unreadable,
}

impl State {
    pub(crate) const fn wire(self) -> &'static str {
        match self {
            Self::Absent => "absent",
            Self::Null => "null",
            Self::Empty | Self::Present => "present",
            Self::Invalid => "invalid",
            Self::Unreadable => "unreadable",
        }
    }
}

pub(crate) fn record(guid: u128, class: &str, path: &str) -> TestRecord {
    TestRecord::new(
        guid,
        "SCItemManufacturer",
        format!("SCItemManufacturer.{class}"),
        path,
    )
}

pub(crate) fn database() -> DataForgeBuilder {
    state_database(Slot::Code, State::Present)
}

pub(crate) fn state_database(slot: Slot, state: State) -> DataForgeBuilder {
    database_with_keys(slot, state, "@shared", "@shared")
}

pub(crate) fn with_keys(name: &str, description: &str) -> DataForgeBuilder {
    database_with_keys(Slot::Code, State::Present, name, description)
}

fn database_with_keys(slot: Slot, state: State, name: &str, description: &str) -> DataForgeBuilder {
    let (local_type, local_value) = localization(slot, state, name, description);
    let mut maker_type = TestStruct::new("SCItemManufacturer");
    let mut maker = record(
        MAKER,
        "Synthetic",
        "libs/foundry/records/scitemmanufacturer/included/maker.xml",
    );
    let code_state = if slot == Slot::Code {
        state
    } else {
        State::Present
    };
    if let Some(ty) = declaration(Slot::Code, code_state) {
        maker_type = maker_type.field("Code", ty);
        maker = maker.value("Code", scalar(Slot::Code, code_state, ""));
    }
    let parent_state = if slot == Slot::Localization {
        state
    } else {
        State::Present
    };
    if let Some(ty) = declaration(Slot::Localization, parent_state) {
        maker_type = maker_type.field("Localization", ty);
        let value = match parent_state {
            State::Null => TestValue::Null,
            State::Invalid => TestValue::Bool(false),
            State::Unreadable => invalid_pointer(),
            _ => TestValue::Struct(local_value),
        };
        maker = maker.value("Localization", value);
    }
    DataForgeBuilder::new()
        .version(8)
        .structure(maker_type)
        .structure(local_type)
        .structure(TestStruct::new("Empty"))
        .record(maker)
}

fn localization(
    slot: Slot,
    state: State,
    name_key: &str,
    description_key: &str,
) -> (TestStruct, TestInstance) {
    let mut source = TestStruct::new("ManufacturerLocalization");
    let mut instance = TestInstance::new("ManufacturerLocalization");
    if slot == Slot::Localization && state == State::Empty {
        return (source, instance);
    }
    for (name, field) in [("Name", Slot::Name), ("Description", Slot::Description)] {
        let state = if slot == field { state } else { State::Present };
        if let Some(ty) = declaration(field, state) {
            source = source.field(name, ty);
            instance = instance.value(
                name,
                scalar(
                    field,
                    state,
                    if field == Slot::Name {
                        name_key
                    } else {
                        description_key
                    },
                ),
            );
        }
    }
    (source, instance)
}

fn declaration(slot: Slot, state: State) -> Option<TestType> {
    match state {
        State::Absent => None,
        State::Null => Some(TestType::StrongPointer("Empty".to_owned())),
        State::Invalid => Some(TestType::Bool),
        _ => Some(match slot {
            Slot::Code => TestType::String,
            Slot::Localization => TestType::StrongPointer("ManufacturerLocalization".to_owned()),
            Slot::Name | Slot::Description => TestType::Locale,
        }),
    }
}

fn scalar(slot: Slot, state: State, key: &str) -> TestValue {
    match state {
        State::Null => TestValue::Null,
        State::Invalid => TestValue::Bool(false),
        State::Unreadable => TestValue::Raw((u32::MAX - 1).to_le_bytes().to_vec()),
        State::Empty => TestValue::text(""),
        _ => TestValue::text(if slot == Slot::Code { " SYN " } else { key }),
    }
}

fn invalid_pointer() -> TestValue {
    let mut bytes = (u32::MAX - 1).to_le_bytes().to_vec();
    bytes.extend_from_slice(&(u32::MAX - 1).to_le_bytes());
    TestValue::Raw(bytes)
}

pub(crate) fn build(
    database: &DataForgeBuilder,
    origin: DcbOrigin,
    mode: CatalogPublicationMode,
    filter: Option<&Filter>,
) -> TestResult<(BuiltCatalog, Value)> {
    let (_directory, archive) = archive_fixture::open(database, origin)?;
    let mut options = archive_fixture::options();
    options.publication_mode = mode;
    options.filter = filter;
    let built = build_catalog(&archive, &options)?;
    let catalog = serde_json::from_slice(built.json())?;
    Ok((built, catalog))
}

pub(crate) fn normal(database: &DataForgeBuilder) -> TestResult<(BuiltCatalog, Value)> {
    build(
        database,
        DcbOrigin::Plain,
        CatalogPublicationMode::Normal,
        None,
    )
}

pub(crate) fn rows(catalog: &Value) -> TestResult<&Vec<Value>> {
    at(catalog, "/definitions/manufacturers/rows")?
        .as_array()
        .ok_or_else(|| "manufacturer rows".into())
}

pub(crate) fn row(catalog: &Value) -> TestResult<&Value> {
    rows(catalog)?
        .first()
        .ok_or_else(|| "the manufacturer definition".into())
}

pub(crate) fn text<'catalog>(
    catalog: &'catalog Value,
    domain: &str,
    field: &str,
) -> TestResult<&'catalog Value> {
    let reference = at(row(catalog)?, &format!("/localization/value/{field}/value"))?;
    text_row(catalog, domain, reference)
}
