// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, Language};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue,
};

use crate::commodity_v3_fixture::{DcbOrigin, TestResult};

pub(super) const OWNER: u128 = 0x9867;
const ENTITY: &str = "EntityClassDefinition";
const COMPONENT: &str = "DataForgeComponentParams";
const ATTACHMENT: &str = "SAttachableComponentParams";
const DEFINITION: &str = "SItemDefinition";
const FOREIGN: &str = "FixtureForeignDefinition";
const LOCALIZATION: &str = "SCItemLocalization";
const PHYSICS: &str = "SEntityPhysicsControllerParams";
const RIGID: &str = "SEntityRigidPhysicsControllerParams";
const PURCHASE: &str = "SCItemPurchasableParams";
const LABELS: &str = "FixtureLabels";
const DEFINITION_INDEX: u32 = 3;
const ATTACHMENT_INDEX: u32 = 2;

#[derive(Debug, Clone, Copy)]
pub(super) enum Layout {
    Normal,
    Foreign,
    NullFirst,
    UnreadableFirst,
    UnknownPrefix,
    UnknownSuffix,
    InvalidMass,
    InvalidShopName,
    InvalidName,
    InvalidUnusedName,
    LocalePointer,
    Aliases,
    Player,
    Mission,
    MissingName,
}

pub(super) fn open(
    layout: Layout,
    origin: DcbOrigin,
    english_encrypted: bool,
    german_encrypted: bool,
) -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("item-source.p4k");
    let dcb = TestEntry::stored("Data/Game2.dcb", database(layout).build());
    let dcb = match origin {
        DcbOrigin::Plain => dcb,
        DcbOrigin::Encrypted => dcb.encrypted(),
        DcbOrigin::Unknown => dcb.encrypted().with_encryption_field(7),
    };
    let english = TestEntry::stored(
        Language::English.global_ini_path(),
        b"first=Class: Military\nsecond=Class: Stealth\nname=Fixture name\nshop=Fixture shop\n",
    );
    let german = TestEntry::stored(
        Language::German.global_ini_path(),
        b"first=Class: Civilian\nsecond=Class: Competition\nname=Fixture Name\n",
    );
    ArchiveBuilder::new()
        .entry(dcb)
        .entry(if english_encrypted {
            english.encrypted()
        } else {
            english
        })
        .entry(if german_encrypted {
            german.encrypted()
        } else {
            german
        })
        .entry(TestEntry::stored(
            "Data/Libs/CosmoTools/LayerRecovery/LayerBackups.xml",
            b"<LayerBackups/>",
        ))
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    Ok((folder, archive))
}

pub(super) fn database(layout: Layout) -> DataForgeBuilder {
    let definition_name = definition_type(layout);
    let definitions = [
        TestStruct::new(ENTITY).array("Components", pointer(COMPONENT)),
        TestStruct::new(COMPONENT),
        TestStruct::new(ATTACHMENT)
            .parent(COMPONENT)
            .field("AttachDef", pointer(definition_name)),
        definition(layout),
        TestStruct::new(LOCALIZATION)
            .field(
                "Name",
                if matches!(layout, Layout::InvalidName | Layout::InvalidUnusedName) {
                    TestType::String
                } else {
                    TestType::Locale
                },
            )
            .field("Description", TestType::Locale),
        TestStruct::new(PHYSICS)
            .parent(COMPONENT)
            .field("PhysType", class(RIGID)),
        TestStruct::new(RIGID).field(
            "Mass",
            if matches!(layout, Layout::InvalidMass) {
                TestType::String
            } else {
                TestType::Single
            },
        ),
        TestStruct::new(PURCHASE).parent(COMPONENT).field(
            "displayName",
            if matches!(layout, Layout::InvalidShopName) {
                TestType::String
            } else {
                TestType::Locale
            },
        ),
    ];
    let mut database = definitions
        .into_iter()
        .fold(
            DataForgeBuilder::new().version(8).enumeration(
                LABELS,
                [
                    "FixtureType",
                    "FixtureSub",
                    "TagOne TagTwo",
                    "RequiredOne RequiredTwo",
                ],
            ),
            DataForgeBuilder::structure,
        )
        .record(owner("Alpha", "@first", layout));
    if matches!(layout, Layout::Aliases) {
        database = database.record(owner("Beta", "@second", layout));
    }
    database
}

fn definition(layout: Layout) -> TestStruct {
    let text = if matches!(layout, Layout::Foreign) {
        TestType::Enum(LABELS.to_owned())
    } else {
        TestType::String
    };
    let localization = if matches!(layout, Layout::LocalePointer) {
        pointer(LOCALIZATION)
    } else {
        class(LOCALIZATION)
    };
    TestStruct::new(definition_type(layout))
        .field("Type", text.clone())
        .field("SubType", text.clone())
        .field(
            "Size",
            if matches!(layout, Layout::Foreign) {
                TestType::UInt64
            } else {
                TestType::Int32
            },
        )
        .field("Tags", text.clone())
        .field("RequiredTags", text)
        .field("Grade", TestType::Int32)
        .field("Localization", localization)
}

fn owner(name: &str, description: &str, layout: Layout) -> TestRecord {
    let first = match layout {
        Layout::NullFirst => TestValue::Null,
        Layout::UnreadableFirst => bad_pointer(DEFINITION_INDEX),
        _ => definition_value(description, layout),
    };
    let mut components = vec![TestValue::Null];
    if matches!(layout, Layout::UnknownPrefix) {
        components.push(bad_pointer(ATTACHMENT_INDEX));
    }
    components.extend([
        node(TestInstance::new(ATTACHMENT).value("AttachDef", first)),
        node(TestInstance::new(ATTACHMENT).value("AttachDef", definition_value("@second", layout))),
        node(TestInstance::new(PHYSICS).value(
            "PhysType",
            node(TestInstance::new(RIGID).value(
                "Mass",
                if matches!(layout, Layout::InvalidMass) {
                    TestValue::text("not a mass")
                } else {
                    TestValue::Single(-0.0)
                },
            )),
        )),
        node(TestInstance::new(PURCHASE).value(
            "displayName",
            if matches!(layout, Layout::InvalidShopName) {
                TestValue::text("opaque shop")
            } else if matches!(layout, Layout::Player | Layout::InvalidUnusedName) {
                TestValue::text("@shop")
            } else {
                TestValue::text("")
            },
        )),
    ]);
    if matches!(layout, Layout::UnknownSuffix) {
        components.push(bad_pointer(ATTACHMENT_INDEX));
    }
    let path = if matches!(layout, Layout::Mission) {
        "mission_entities"
    } else {
        "fixture"
    };
    TestRecord::new(
        OWNER,
        ENTITY,
        format!("{ENTITY}.{name}"),
        format!("libs/foundry/records/entities/scitem/{path}/item_source_{name}.xml"),
    )
    .value("Components", TestValue::Array(components))
}

fn definition_value(description: &str, layout: Layout) -> TestValue {
    let size = if matches!(layout, Layout::Foreign) {
        TestValue::UInt64(u64::MAX)
    } else {
        TestValue::Int32(2)
    };
    let name = if matches!(layout, Layout::MissingName) {
        TestValue::text("")
    } else {
        TestValue::text("@name")
    };
    node(
        TestInstance::new(definition_type(layout))
            .value("Type", TestValue::text("FixtureType"))
            .value("SubType", TestValue::text("FixtureSub"))
            .value("Size", size)
            .value("Grade", TestValue::Int32(3))
            .value("Tags", TestValue::text("TagOne TagTwo"))
            .value("RequiredTags", TestValue::text("RequiredOne RequiredTwo"))
            .value(
                "Localization",
                node(
                    TestInstance::new(LOCALIZATION)
                        .value("Name", name)
                        .value("Description", TestValue::text(description)),
                ),
            ),
    )
}

const fn definition_type(layout: Layout) -> &'static str {
    if matches!(layout, Layout::Foreign) {
        FOREIGN
    } else {
        DEFINITION
    }
}

fn bad_pointer(structure: u32) -> TestValue {
    let mut bytes = structure.to_le_bytes().to_vec();
    bytes.extend_from_slice(&(u32::MAX - 1).to_le_bytes());
    TestValue::Raw(bytes)
}

const fn node(instance: TestInstance) -> TestValue {
    TestValue::Struct(instance)
}
fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}
fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}
