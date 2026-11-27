// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Synthetic Actor owners, driver seats, and control Records for the vehicle catalog.

use p4kx_testkit::{
    DataForgeBuilder, TestEntity, TestInstance, TestLoadout, TestLoadoutEntry, TestRecord,
    TestStruct, TestType, TestValue,
};

const ACTOR_DIRECTORY: &str = "libs/foundry/records/actor/actors";
const SEAT_DIRECTORY: &str = "libs/foundry/records/entities/scitem/vehicles/seat";
const ACTOR_COMPONENT_TYPES: [&str; 4] = [
    "SActorComponentParams",
    "SSCItemControllableManagerParams",
    "ItemControlComponentParams",
    "ItemControllerComponentParams",
];
const SEAT: &str = "SCItemSeatParams";
const SEAT_TYPE: &str = "SeatTypes";
const ATTACHMENT: &str = "SCSeatActorAttachmentParams";
const ATTACHMENT_TYPE: &str = "SCSeatActorAttachmentType";
const MODE: &str = "OperatorModeAvailabilityParams";

#[derive(Clone, Copy)]
struct Driver {
    guid: u128,
    class: &'static str,
}

const ATLS_DRIVER: Driver = Driver {
    guid: 0x1182_0020,
    class: "argo_atls_powersuit_scitem_seat_driver",
};
const GEO_DRIVER: Driver = Driver {
    guid: 0x1182_0021,
    class: "argo_atls_geo_scitem_seat_driver",
};
const IKTI_DRIVER: Driver = Driver {
    guid: 0x1182_0022,
    class: "argo_atls_ikti_scitem_seat_driver",
};
const ACCESSORY: Driver = Driver {
    guid: 0x1182_0023,
    class: "FixtureSeatAccessory",
};

const MODE_GUID: u128 = 0x1182_0030;
pub(super) const ALIASED_OWNER: u128 = 0x1182_0001;
pub(super) const GEO_OWNER_GUID: u128 = 0x1182_0002;

#[derive(Clone, Copy)]
pub(super) struct Manufacturer {
    pub(super) guid: u128,
    pub(super) code: &'static str,
}

const ARGO: Manufacturer = Manufacturer {
    guid: 0x1182_0040,
    code: "ARGO",
};
const BANU: Manufacturer = Manufacturer {
    guid: 0x1182_0041,
    code: "BANU",
};
const UNKN: Manufacturer = Manufacturer {
    guid: 0x1182_0042,
    code: "UNKN",
};

pub(super) struct Owner {
    pub(super) guid: u128,
    pub(super) class: &'static str,
    pub(super) name_key: &'static str,
    pub(super) manufacturer: Option<Manufacturer>,
    driver: Driver,
}

impl Owner {
    pub(super) const fn driver_class(&self) -> &'static str {
        self.driver.class
    }
}

pub(super) const OWNERS: [Owner; 9] = [
    Owner {
        guid: ALIASED_OWNER,
        class: "ARGO_ATLS",
        name_key: "@fixture_atls",
        manufacturer: None,
        driver: ATLS_DRIVER,
    },
    Owner {
        guid: GEO_OWNER_GUID,
        class: "ARGO_ATLS_GEO",
        name_key: "@fixture_atls_geo",
        manufacturer: Some(ARGO),
        driver: GEO_DRIVER,
    },
    Owner {
        guid: 0x1182_0003,
        class: "ARGO_ATLS_IKTI",
        name_key: "@fixture_atls_ikti",
        manufacturer: Some(BANU),
        driver: IKTI_DRIVER,
    },
    Owner {
        guid: 0x1182_0004,
        class: "ARGO_ATLS_GEO_IKTI",
        name_key: "@fixture_atls_ikti_geo",
        manufacturer: Some(BANU),
        driver: IKTI_DRIVER,
    },
    Owner {
        guid: 0x1182_0005,
        class: "ARGO_ATLS_GEO_IKTI_NTLS",
        name_key: "@fixture_atls_ikti_akuma",
        manufacturer: Some(UNKN),
        driver: IKTI_DRIVER,
    },
    Owner {
        guid: 0x1182_0006,
        class: "ARGO_ATLS_IKTI_ARGOS",
        name_key: "@fixture_atls_ikti_rad",
        manufacturer: Some(BANU),
        driver: IKTI_DRIVER,
    },
    Owner {
        guid: 0x1182_0007,
        class: "ARGO_ATLS_GEO_COLLECTOR_GRAD01",
        name_key: "@fixture_atls_snowland",
        manufacturer: Some(ARGO),
        driver: GEO_DRIVER,
    },
    Owner {
        guid: 0x1182_0008,
        class: "ARGO_ATLS_GEO_COLLECTOR_GRAD02",
        name_key: "@fixture_atls_orange_line",
        manufacturer: Some(ARGO),
        driver: GEO_DRIVER,
    },
    Owner {
        guid: 0x1182_0009,
        class: "ARGO_ATLS_GEO_COLLECTOR_GRAD03",
        name_key: "@fixture_atls_cool_metal",
        manufacturer: Some(ARGO),
        driver: GEO_DRIVER,
    },
];

pub(super) fn database() -> DataForgeBuilder {
    database_with_geo(GeoOwner::Present, MODE_GUID)
}

pub(super) fn database_with_unreadable_geo_mode() -> DataForgeBuilder {
    database_with_geo(GeoOwner::Present, 0x1182_ffff)
}

#[derive(Clone, Copy)]
pub(super) enum GeoOwner {
    Present,
    Absent,
    Unreadable,
}

pub(super) fn database_with_geo_owner(owner: GeoOwner) -> DataForgeBuilder {
    database_with_geo(owner, MODE_GUID)
}

fn database_with_geo(geo: GeoOwner, mode: u128) -> DataForgeBuilder {
    let mut source = declarations();
    for driver in [ATLS_DRIVER, GEO_DRIVER, IKTI_DRIVER, ACCESSORY] {
        let mode = if driver.guid == GEO_DRIVER.guid {
            mode
        } else {
            MODE_GUID
        };
        source = source.record(driver_record(driver, mode));
    }
    source = source.record(TestRecord::new(
        MODE_GUID,
        MODE,
        "OperatorModeAvailability_Fixture",
        "libs/foundry/records/operatormodes/fixture",
    ));
    for manufacturer in [ARGO, BANU, UNKN] {
        let code = manufacturer.code;
        source = source.record(
            TestRecord::new(
                manufacturer.guid,
                "SCItemManufacturer",
                format!("SCItemManufacturer.{code}"),
                format!("libs/foundry/records/scitemmanufacturer/{code}.xml"),
            )
            .value("Code", TestValue::text(code)),
        );
    }
    for owner in &OWNERS {
        let record = owner_record(owner);
        let record = if owner.guid == GEO_OWNER_GUID {
            match geo {
                GeoOwner::Present => record,
                GeoOwner::Absent => continue,
                GeoOwner::Unreadable => record.with_instance_index(u16::MAX),
            }
        } else {
            record
        };
        source = source.record(record);
    }
    source
        .record(owner_record(&OWNERS[0]))
        .record(actor(0x1182_0010, "FixtureUnrelatedActor", ATLS_DRIVER).record())
        .record(actor(0x1182_0011, "ARGO_ATLS_GEO_Graded", GEO_DRIVER).record())
        .record(actor(0x1182_0012, "ARGO_ATLS_GEO_IKTI", ACCESSORY).record())
        .record(actor(0x1182_0013, "ARGO_ATLS_IKTI_ARGOS_Test", IKTI_DRIVER).record())
}

fn entity_declarations() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("EntityClassDefinition")
                .field("Category", TestType::String)
                .array(
                    "Components",
                    TestType::StrongPointer("DataForgeComponentParams".to_owned()),
                ),
        )
        .structure(TestStruct::new("DataForgeComponentParams"))
        .structure(
            TestStruct::new("SEntityComponentDefaultLoadoutParams")
                .parent("DataForgeComponentParams")
                .field(
                    "loadout",
                    TestType::StrongPointer("SItemPortLoadoutBaseParams".to_owned()),
                ),
        )
        .structure(TestStruct::new("SItemPortLoadoutBaseParams"))
        .structure(
            TestStruct::new("SItemPortLoadoutManualParams")
                .parent("SItemPortLoadoutBaseParams")
                .array(
                    "entries",
                    TestType::Class("SItemPortLoadoutEntryParams".to_owned()),
                ),
        )
        .structure(
            TestStruct::new("SItemPortLoadoutEntryParams")
                .field("itemPortName", TestType::String)
                .field(
                    "entityClassReference",
                    TestType::ReferenceTo("EntityClassDefinition".to_owned()),
                ),
        )
}

fn declarations() -> DataForgeBuilder {
    let mut source = entity_declarations()
        .structure(
            TestStruct::new("SAttachableComponentParams")
                .parent("DataForgeComponentParams")
                .field("AttachDef", TestType::Class("SItemDefinition".to_owned())),
        )
        .structure(
            TestStruct::new("SItemDefinition")
                .field("Type", TestType::String)
                .field("SubType", TestType::String)
                .field(
                    "Manufacturer",
                    TestType::ReferenceTo("SCItemManufacturer".to_owned()),
                )
                .field(
                    "Localization",
                    TestType::Class("SCItemLocalization".to_owned()),
                ),
        )
        .structure(
            TestStruct::new("SCItemLocalization")
                .field("Name", TestType::Locale)
                .field("ShortName", TestType::Locale),
        )
        .structure(TestStruct::new("SCItemManufacturer").field("Code", TestType::String))
        .enumeration(SEAT_TYPE, ["HOTAS_C_L"])
        .enumeration(ATTACHMENT_TYPE, ["Host"])
        .structure(
            TestStruct::new(SEAT)
                .parent("DataForgeComponentParams")
                .field("seatType", TestType::Enum(SEAT_TYPE.to_owned()))
                .field("controlTemplate", TestType::String)
                .field("userAnimationDatabase", TestType::String)
                .field("actorAttachment", TestType::Class(ATTACHMENT.to_owned()))
                .field(
                    "availableOperatorModes",
                    TestType::ReferenceTo(MODE.to_owned()),
                ),
        )
        .structure(
            TestStruct::new(ATTACHMENT).field("type", TestType::Enum(ATTACHMENT_TYPE.to_owned())),
        )
        .structure(TestStruct::new(MODE));
    for component in ACTOR_COMPONENT_TYPES {
        source = source.structure(TestStruct::new(component).parent("DataForgeComponentParams"));
    }
    source
}

fn actor(guid: u128, class: &str, driver: Driver) -> TestEntity {
    let path = format!("{ACTOR_DIRECTORY}/{}", class.to_ascii_lowercase());
    let loadout = TestLoadout::Manual(vec![TestLoadoutEntry::reference(
        "seat_driver",
        driver.guid,
    )]);
    let mut entity = TestEntity::new(guid, class, path).loadout(loadout);
    for component in ACTOR_COMPONENT_TYPES {
        entity = entity.component(TestValue::Struct(TestInstance::new(component)));
    }
    entity
}

fn owner_record(owner: &Owner) -> TestRecord {
    let localization = TestInstance::new("SCItemLocalization")
        .value("Name", TestValue::text(owner.name_key))
        .value("ShortName", TestValue::text("@fixture_shared_short_name"));
    let manufacturer = owner
        .manufacturer
        .map_or(TestValue::Null, |maker| TestValue::Reference(maker.guid));
    let definition = TestInstance::new("SItemDefinition")
        .value("Type", TestValue::text("NOITEM_Vehicle"))
        .value("SubType", TestValue::text("Vehicle_PowerSuit"))
        .value("Manufacturer", manufacturer)
        .value("Localization", TestValue::Struct(localization));
    actor(owner.guid, owner.class, owner.driver)
        .component(TestValue::Struct(
            TestInstance::new("SAttachableComponentParams")
                .value("AttachDef", TestValue::Struct(definition)),
        ))
        .record()
}

fn driver_record(driver: Driver, mode: u128) -> TestRecord {
    TestEntity::new(
        driver.guid,
        driver.class,
        format!("{SEAT_DIRECTORY}/{}", driver.class.to_ascii_lowercase()),
    )
    .component(TestValue::Struct(
        TestInstance::new(SEAT)
            .value("seatType", TestValue::text("HOTAS_C_L"))
            .value("controlTemplate", TestValue::text(""))
            .value(
                "userAnimationDatabase",
                TestValue::text("Animations/Fixture/seated.adb"),
            )
            .value(
                "actorAttachment",
                TestValue::Struct(
                    TestInstance::new(ATTACHMENT).value("type", TestValue::text("Host")),
                ),
            )
            .value("availableOperatorModes", TestValue::Reference(mode)),
    ))
    .record()
}
