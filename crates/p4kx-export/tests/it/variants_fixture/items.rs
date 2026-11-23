// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The items of the variants fixture: pairs that differ in exactly one thing each.

use p4kx_testkit::{TestInstance, TestRecord, TestStruct, TestType, TestValue};

use super::{INVALID_COMPONENT_TYPE, component, guid};

pub(super) fn structs() -> Vec<TestStruct> {
    vec![
        TestStruct::new("SCItemLocalization").field("Name", TestType::Locale),
        component("SAttachableComponentParams")
            .field("AttachDef", TestType::Class("SItemDefinition".to_owned())),
        TestStruct::new("SItemDefinition")
            .field("Type", TestType::Enum("EItemType".to_owned()))
            .field("Size", TestType::Int32)
            .field("Grade", TestType::Int32)
            .field("Tags", TestType::String)
            .field(
                "Localization",
                TestType::Class("SCItemLocalization".to_owned()),
            ),
        component("SEntityPhysicsControllerParams")
            .field("PhysType", TestType::Class("SEntityPhysicsType".to_owned())),
        TestStruct::new("SEntityPhysicsType").field("Mass", TestType::Single),
        component("SEntityComponentCarryableParams").field("carryInteraction", TestType::String),
        component("SEntityInteractableParams").field("value", TestType::String),
        component("SInteractionStateMachineParams").field("value", TestType::String),
        component("SActorUsableParams").field("value", TestType::String),
        component("SCItemInventoryContainerComponentParams")
            .field("openInventoryInteraction", TestType::String)
            .field("openLootingInteraction", TestType::String),
        component("SGeometryResourceParams").field(
            "Geometry",
            TestType::Class("SGeometryNodeParams".to_owned()),
        ),
        TestStruct::new("SGeometryNodeParams")
            .field("Geometry", TestType::Class("SPathRef".to_owned()))
            .field("Material", TestType::Class("SPathRef".to_owned()))
            .field("Materials", TestType::String)
            .field("Palette", TestType::String),
        TestStruct::new("SPathRef").field("path", TestType::String),
    ]
}

#[derive(Clone, Copy)]
struct Model {
    file: &'static str,
    material: &'static str,
    materials: &'static str,
    palette: &'static str,
}

#[derive(Clone, Copy)]
struct Probe {
    component: &'static str,
    field: &'static str,
    value: &'static str,
}

#[derive(Clone, Copy)]
struct Item {
    class: &'static str,
    mass: Option<f32>,
    grade: i32,
    tags: &'static str,
    carry: &'static str,
    model: Option<Model>,
    probe: Option<Probe>,
}

const fn base(class: &'static str) -> Item {
    Item {
        class,
        mass: Some(10.0),
        grade: 1,
        tags: "red",
        carry: "carry",
        model: None,
        probe: None,
    }
}

const fn chair(class: &'static str, model: &'static str, material: &'static str) -> Item {
    Item {
        mass: None,
        model: Some(Model {
            file: model,
            material,
            materials: "",
            palette: material,
        }),
        ..base(class)
    }
}

const fn textured(class: &'static str, model: Model) -> Item {
    Item {
        mass: None,
        model: Some(model),
        ..base(class)
    }
}

const fn probed(
    class: &'static str,
    grade: i32,
    component: &'static str,
    field: &'static str,
    value: &'static str,
) -> Item {
    Item {
        grade,
        probe: Some(Probe {
            component,
            field,
            value,
        }),
        ..base(class)
    }
}

const fn plain(file: &'static str) -> Model {
    Model {
        file,
        material: "m.mtl",
        materials: "",
        palette: "p",
    }
}

const INVENTORY: &str = "SCItemInventoryContainerComponentParams";

const ITEMS: &[Item] = &[
    chair("PROP_Chair_Red", "chair.cgf", "red.mtl"),
    chair("PROP_Chair_Blue", "chair.cgf", "blue.mtl"),
    chair("PROP_Table", "table.cgf", "red.mtl"),
    chair("PROP_Bench", "bench.cgf", "red.mtl"),
    base("POWR_Alpha"),
    Item {
        tags: "blue",
        ..base("POWR_Alpha_Blue")
    },
    Item {
        mass: Some(11.0),
        ..base("POWR_Heavy")
    },
    Item {
        grade: 2,
        ..base("POWR_Grade2")
    },
    Item {
        mass: None,
        ..base("POWR_Bare_One")
    },
    Item {
        mass: None,
        tags: "blue",
        ..base("POWR_Bare_Two")
    },
    Item {
        grade: 3,
        ..base("POWR_Tags_A")
    },
    Item {
        grade: 3,
        tags: "blue",
        ..base("POWR_Tags_B")
    },
    Item {
        grade: 4,
        ..base("POWR_Carry_A")
    },
    Item {
        grade: 4,
        carry: "other",
        ..base("POWR_Carry_B")
    },
    probed(
        "POWR_Interact_A",
        5,
        "SEntityInteractableParams",
        "value",
        "a",
    ),
    probed(
        "POWR_Interact_B",
        5,
        "SEntityInteractableParams",
        "value",
        "b",
    ),
    probed(
        "POWR_State_A",
        6,
        "SInteractionStateMachineParams",
        "value",
        "a",
    ),
    probed(
        "POWR_State_B",
        6,
        "SInteractionStateMachineParams",
        "value",
        "b",
    ),
    probed("POWR_Usable_A", 7, "SActorUsableParams", "value", "a"),
    probed("POWR_Usable_B", 7, "SActorUsableParams", "value", "b"),
    probed("POWR_Open_A", 8, INVENTORY, "openInventoryInteraction", "a"),
    probed("POWR_Open_B", 8, INVENTORY, "openInventoryInteraction", "b"),
    probed("POWR_Loot_A", 9, INVENTORY, "openLootingInteraction", "a"),
    probed("POWR_Loot_B", 9, INVENTORY, "openLootingInteraction", "b"),
    textured("PROP_Material_A", plain("one.cgf")),
    textured(
        "PROP_Material_B",
        Model {
            material: "n.mtl",
            ..plain("one.cgf")
        },
    ),
    textured("PROP_Materials_A", plain("two.cgf")),
    textured(
        "PROP_Materials_B",
        Model {
            materials: "set",
            ..plain("two.cgf")
        },
    ),
    textured("PROP_Palette_A", plain("three.cgf")),
    textured(
        "PROP_Palette_B",
        Model {
            palette: "q",
            ..plain("three.cgf")
        },
    ),
];

fn geometry(model: Model) -> TestValue {
    let path = |value: &str| {
        TestValue::Struct(TestInstance::new("SPathRef").value("path", TestValue::text(value)))
    };
    TestValue::Struct(
        TestInstance::new("SGeometryResourceParams").value(
            "Geometry",
            TestValue::Struct(
                TestInstance::new("SGeometryNodeParams")
                    .value("Geometry", path(model.file))
                    .value("Material", path(model.material))
                    .value("Materials", TestValue::text(model.materials))
                    .value("Palette", TestValue::text(model.palette)),
            ),
        ),
    )
}

fn attach(index: usize, item: &Item) -> TestValue {
    TestValue::Struct(
        TestInstance::new("SAttachableComponentParams").value(
            "AttachDef",
            TestValue::Struct(
                TestInstance::new("SItemDefinition")
                    .value("Type", TestValue::text("PowerPlant"))
                    .value("Size", TestValue::Int32(1))
                    .value("Grade", TestValue::Int32(item.grade))
                    .value("Tags", TestValue::text(item.tags))
                    .value(
                        "Localization",
                        TestValue::Struct(
                            TestInstance::new("SCItemLocalization")
                                .value("Name", TestValue::text(format!("@item_Name{index}"))),
                        ),
                    ),
            ),
        ),
    )
}

fn item_record(index: usize, item: &Item, trailing_component: Option<TestValue>) -> TestRecord {
    let mut components = vec![
        attach(index, item),
        TestValue::Struct(
            TestInstance::new("SEntityComponentCarryableParams")
                .value("carryInteraction", TestValue::text(item.carry)),
        ),
    ];
    if let Some(model) = item.model {
        components.push(geometry(model));
    }
    if let Some(probe) = item.probe {
        components.push(TestValue::Struct(
            TestInstance::new(probe.component).value(probe.field, TestValue::text(probe.value)),
        ));
    }
    if let Some(mass) = item.mass {
        components.push(TestValue::Struct(
            TestInstance::new("SEntityPhysicsControllerParams").value(
                "PhysType",
                TestValue::Struct(
                    TestInstance::new("SEntityPhysicsType").value("Mass", TestValue::Single(mass)),
                ),
            ),
        ));
    }
    components.extend(trailing_component);
    TestRecord::new(
        guid(0x100 + index as u128),
        "EntityClassDefinition",
        format!("EntityClassDefinition.{}", item.class),
        format!(
            "libs/foundry/records/entities/scitem/ships/powerplant/{}.xml",
            item.class
        ),
    )
    .value("Components", TestValue::Array(components))
}

pub(super) fn records() -> impl Iterator<Item = TestRecord> {
    ITEMS
        .iter()
        .enumerate()
        .map(|(index, item)| item_record(index, item, None))
}

pub(super) fn key_records(unreadable: bool) -> impl Iterator<Item = TestRecord> {
    let excluded = TestValue::Struct(TestInstance::new("SEntityInteractableParams"));
    let broken = unreadable.then(|| {
        let bytes = [INVALID_COMPONENT_TYPE.to_le_bytes(), 3_u32.to_le_bytes()].concat();
        ("POWR_Key_Unreadable", Some(TestValue::Raw(bytes)))
    });
    [
        ("POWR_Key_Base", None),
        ("POWR_Key_Empty", Some(TestValue::Null)),
        ("POWR_Key_Excluded", Some(excluded)),
    ]
    .into_iter()
    .chain(broken)
    .enumerate()
    .map(|(index, (class, slot))| item_record(index, &base(class), slot))
}
