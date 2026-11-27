// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{TestInstance, TestRecord, TestStruct, TestType, TestValue};

/// A geometry given to the sub-node tagged `tags`: its model path and material library. Tags of
/// the form `parent>own` name a sub-node tagged `own` nested in a sub-node tagged `parent`. The
/// tint palette Record and child it names go last.
pub(crate) type GeometryVariant = (String, String, Option<String>, Option<(u128, String)>);

fn geometry_node(
    tags: &str,
    path: &str,
    material: Option<&str>,
    palette: Option<&(u128, String)>,
) -> TestInstance {
    let resource = |type_name: &str, path: &str| {
        TestValue::Struct(TestInstance::new(type_name).value("path", TestValue::text(path)))
    };
    let mut data = TestInstance::new("SGeometryDataParams")
        .value("Geometry", resource("GlobalResourceGeometry", path));
    data = data.value(
        "Material",
        resource("GlobalResourceMaterial", material.unwrap_or_default()),
    );
    if let Some((guid, child)) = palette {
        data = data.value(
            "Palette",
            TestValue::Struct(
                TestInstance::new("TintPaletteRef")
                    .value("ChildPath", TestValue::text(child.clone()))
                    .value("RootRecord", TestValue::Reference(*guid)),
            ),
        );
    }
    TestInstance::new("SGeometryNodeParams")
        .value("Tags", TestValue::text(tags))
        .value("Geometry", TestValue::Struct(data))
}

/// A material library given to the node of the material tree tagged `tags`; tags of the form
/// `parent>own` name a node tagged `own` nested in a node tagged `parent`.
pub(crate) type MaterialChoice = (String, String);

fn material_node(tags: &str, material: &str, nested: Vec<TestValue>) -> TestInstance {
    TestInstance::new("SMaterialNodeParams")
        .value("Tags", TestValue::text(tags))
        .value(
            "Material",
            TestValue::Struct(
                TestInstance::new("GlobalResourceMaterial")
                    .value("path", TestValue::text(material)),
            ),
        )
        .value("materialVariants", TestValue::Array(nested))
}

fn material_tree(choices: &[MaterialChoice]) -> TestValue {
    let nodes = choices
        .iter()
        .map(|(tags, material)| {
            TestValue::Struct(match tags.split_once('>') {
                Some((parent, own)) => material_node(
                    parent,
                    "",
                    vec![TestValue::Struct(material_node(own, material, Vec::new()))],
                ),
                None => material_node(tags, material, Vec::new()),
            })
        })
        .collect();
    TestValue::Struct(material_node("", "", nodes))
}

pub(crate) fn geometry_component(
    path: &str,
    material: Option<&str>,
    variants: &[GeometryVariant],
    palette: Option<&(u128, String)>,
    choices: &[MaterialChoice],
) -> TestValue {
    let subs = variants
        .iter()
        .map(|(tags, path, material, palette)| {
            let node = match tags.split_once('>') {
                Some((parent, own)) => geometry_node(parent, "", None, None).value(
                    "SubGeometry",
                    TestValue::Array(vec![TestValue::Struct(geometry_node(
                        own,
                        path,
                        material.as_deref(),
                        palette.as_ref(),
                    ))]),
                ),
                None => geometry_node(tags, path, material.as_deref(), palette.as_ref()),
            };
            TestValue::Struct(node)
        })
        .collect();
    let node =
        geometry_node("", path, material, palette).value("SubGeometry", TestValue::Array(subs));
    TestValue::Struct(
        TestInstance::new("SGeometryResourceParams")
            .value("Geometry", TestValue::Struct(node))
            .value("Material", material_tree(choices)),
    )
}

pub(crate) fn vehicle_structs() -> Vec<TestStruct> {
    vec![
        TestStruct::new("VehicleComponentParams")
            .parent("DataForgeComponentParams")
            .field("vehicleDefinition", TestType::String)
            .field("modification", TestType::String)
            .field(
                "landingSystem",
                TestType::ReferenceTo("VehicleLandingGearSystem".to_owned()),
            )
            .array(
                "objectContainers",
                TestType::Class("SVehicleObjectContainerParams".to_owned()),
            ),
        TestStruct::new("SVehicleObjectContainerParams")
            .field("fileName", TestType::String)
            .field("boneName", TestType::String)
            .field("Offset", TestType::Class("QuatT".to_owned())),
        TestStruct::new("SObjectContainerComponentParams")
            .parent("DataForgeComponentParams")
            .field("objectContainer", TestType::String),
        TestStruct::new("VehicleLandingGearSystem")
            .array("gears", TestType::Class("VehicleLandingGear".to_owned())),
        TestStruct::new("VehicleLandingGear")
            .field("bone", TestType::String)
            .field("mass", TestType::Single)
            .field(
                "geometry",
                TestType::Class("GlobalResourceGeometry".to_owned()),
            ),
    ]
}

/// An object container of a vehicle component: file, bone, position and rotation in degrees.
pub(crate) type ContainerPlacement = (String, String, [f32; 3], [f32; 3]);

pub(crate) fn item_container_component(file: &str) -> TestValue {
    TestValue::Struct(
        TestInstance::new("SObjectContainerComponentParams")
            .value("objectContainer", TestValue::text(file)),
    )
}

fn container_value((file, bone, position, rotation): &ContainerPlacement) -> TestValue {
    let xyz = |type_name: &str, [x, y, z]: [f32; 3]| {
        TestValue::Struct(
            TestInstance::new(type_name)
                .value("x", TestValue::Single(x))
                .value("y", TestValue::Single(y))
                .value("z", TestValue::Single(z)),
        )
    };
    let offset = TestInstance::new("QuatT")
        .value("Rotation", xyz("Ang3", *rotation))
        .value("Position", xyz("Vec3", *position));
    TestValue::Struct(
        TestInstance::new("SVehicleObjectContainerParams")
            .value("fileName", TestValue::text(file.clone()))
            .value("boneName", TestValue::text(bone.clone()))
            .value("Offset", TestValue::Struct(offset)),
    )
}

pub(crate) fn vehicle_component(
    definition: &str,
    modification: &str,
    landing_system: Option<u128>,
    containers: &[ContainerPlacement],
) -> TestValue {
    let mut component = TestInstance::new("VehicleComponentParams")
        .value("vehicleDefinition", TestValue::text(definition))
        .value("modification", TestValue::text(modification))
        .value(
            "objectContainers",
            TestValue::Array(containers.iter().map(container_value).collect()),
        );
    if let Some(guid) = landing_system {
        component = component.value("landingSystem", TestValue::Reference(guid));
    }
    TestValue::Struct(component)
}

/// Returns the Record of type `VehicleLandingGearSystem` named `name` whose gears move the bone
/// of each `(bone, geometry path, mass)`; a vehicle's `landingSystem` refers to it.
#[must_use]
pub fn landing_system_record(guid: u128, name: &str, gears: &[(&str, &str, f32)]) -> TestRecord {
    let gears = gears
        .iter()
        .map(|(bone, geometry, mass)| {
            TestValue::Struct(
                TestInstance::new("VehicleLandingGear")
                    .value("bone", TestValue::text(*bone))
                    .value("mass", TestValue::Single(*mass))
                    .value(
                        "geometry",
                        TestValue::Struct(
                            TestInstance::new("GlobalResourceGeometry")
                                .value("path", TestValue::text(*geometry)),
                        ),
                    ),
            )
        })
        .collect();
    TestRecord::new(
        guid,
        "VehicleLandingGearSystem",
        format!("VehicleLandingGearSystem.{name}"),
        format!("libs/foundry/records/entities/scitem/ships/landingsystem/{name}.xml"),
    )
    .value("gears", TestValue::Array(gears))
}
