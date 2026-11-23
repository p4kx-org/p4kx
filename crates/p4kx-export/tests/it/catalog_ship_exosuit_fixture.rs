// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Synthetic Actor owners, driver seats, and control Records for the vehicle catalog.

use p4kx_testkit::{
    DataForgeBuilder, TestEntity, TestInstance, TestLoadout, TestLoadoutEntry, TestRecord,
    TestStruct, TestType, TestValue, entity_schema,
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

pub(super) struct Owner {
    pub(super) guid: u128,
    pub(super) class: &'static str,
    driver: Driver,
}

impl Owner {
    pub(super) const fn driver_class(&self) -> &'static str {
        self.driver.class
    }
}

pub(super) const OWNERS: [Owner; 4] = [
    Owner {
        guid: ALIASED_OWNER,
        class: "ARGO_ATLS",
        driver: ATLS_DRIVER,
    },
    Owner {
        guid: 0x1182_0002,
        class: "ARGO_ATLS_GEO",
        driver: GEO_DRIVER,
    },
    Owner {
        guid: 0x1182_0003,
        class: "ARGO_ATLS_IKTI",
        driver: IKTI_DRIVER,
    },
    Owner {
        guid: 0x1182_0004,
        class: "ARGO_ATLS_GEO_IKTI",
        driver: IKTI_DRIVER,
    },
];

pub(super) fn database() -> DataForgeBuilder {
    database_with_geo_mode(MODE_GUID)
}

pub(super) fn database_with_unreadable_geo_mode() -> DataForgeBuilder {
    database_with_geo_mode(0x1182_ffff)
}

fn database_with_geo_mode(mode: u128) -> DataForgeBuilder {
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
    for owner in &OWNERS {
        source = source.record(actor(owner.guid, owner.class, owner.driver));
    }
    source
        .record(actor(ALIASED_OWNER, OWNERS[0].class, ATLS_DRIVER))
        .record(actor(0x1182_0010, "FixtureUnrelatedActor", ATLS_DRIVER))
        .record(actor(0x1182_0011, "ARGO_ATLS_GEO_Graded", GEO_DRIVER))
        .record(actor(0x1182_0012, "ARGO_ATLS_GEO_IKTI", ACCESSORY))
}

fn declarations() -> DataForgeBuilder {
    let mut source = entity_schema(DataForgeBuilder::new().version(8))
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

fn actor(guid: u128, class: &str, driver: Driver) -> TestRecord {
    let path = format!("{ACTOR_DIRECTORY}/{}", class.to_ascii_lowercase());
    let loadout = TestLoadout::Manual(vec![TestLoadoutEntry::reference(
        "seat_driver",
        driver.guid,
    )]);
    let mut entity = TestEntity::new(guid, class, path).loadout(loadout);
    for component in ACTOR_COMPONENT_TYPES {
        entity = entity.component(TestValue::Struct(TestInstance::new(component)));
    }
    entity.record()
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
