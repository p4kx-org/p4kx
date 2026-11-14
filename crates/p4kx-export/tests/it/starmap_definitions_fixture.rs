// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(super) const TYPE: u128 = 0x0010_6701;
pub(super) const PRESET: u128 = 0x0010_6702;
pub(super) const SECOND: u128 = 0x0010_6703;
pub(super) const AMENITY: u128 = 0x0010_6704;
pub(super) const REQUIRED_ENTITY: u128 = 0x0010_6705;
pub(super) const RADAR_CONTACT: u128 = 0x0010_6706;
pub(super) const MISSING: u128 = 0x0010_6707;
pub(super) const FUTURE_ICON: &str = "FixtureFutureNavIcon";

const TYPE_DECL: &str = "StarMapObjectType";
const PRESET_DECL: &str = "StarMapPreset";
const AMENITY_DECL: &str = "StarMapAmenityTypeEntry";
const VISIBILITY: &str = "StarMapPresetVisibilitySettings";
const DISPLAY: &str = "StarMapPresetDisplaySettings";
const RADAR: &str = "StarMapPresetRadarSettings";
const RADAR_PROPERTIES: &str = "SSCRadarContactProperites";
const QT: &str = "StarMapQuantumTravelDataParams";
const LOCATION: &str = "StarMapObjectLocationParams";
const ASSET: &str = "StarMapAssetManagerLocationParams";
const ICON: &str = "NavPointIconEnum";
const FACING: &str = "WorldDisplayObjectFacingMode";
const RADAR_TYPE: &str = "RadarContactTypeEntry";
const ENTITY: &str = "LocationEntityDeclaration";

pub(super) fn database() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .enumeration(ICON, [FUTURE_ICON])
        .enumeration(FACING, ["FixtureFacing"])
        .structure(type_declaration())
        .structure(preset_declaration())
        .structure(visibility())
        .structure(display())
        .structure(quantum_travel())
        .structure(location())
        .structure(
            TestStruct::new(ASSET)
                .field("previewImagePath", TestType::String)
                .field("previewIconPath", TestType::String),
        )
        .structure(
            TestStruct::new(RADAR)
                .field("isScannable", TestType::Bool)
                .field("radarProperties", pointer(RADAR_PROPERTIES)),
        )
        .structure(
            TestStruct::new(RADAR_PROPERTIES)
                .field("contactType", reference(RADAR_TYPE))
                .field("fixtureFutureRadarBits", TestType::Single),
        )
        .structure(
            TestStruct::new(AMENITY_DECL)
                .field("name", TestType::String)
                .field("displayName", TestType::Locale)
                .field("icon", TestType::String)
                .field("requiredEntity", reference(ENTITY)),
        )
        .structure(TestStruct::new("Marker_Configuration"))
        .structure(TestStruct::new("EntityClassDefinition"))
        .structure(TestStruct::new("GlobalResourceGeometry").field("path", TestType::String))
        .structure(TestStruct::new("GlobalResourceMaterial").field("path", TestType::String))
        .structure(TestStruct::new(ENTITY))
        .structure(TestStruct::new(RADAR_TYPE))
        .record(object_type())
        .record(preset())
        .record(
            record(SECOND, PRESET_DECL, "selected/preset/second.xml")
                .value("type", TestValue::Reference(TYPE))
                .value("quantumTravelData", TestValue::Null),
        )
        .record(
            record(AMENITY, AMENITY_DECL, "outside/amenity.xml")
                .value("name", TestValue::text("FixtureAmenity"))
                .value("displayName", TestValue::text("@shared"))
                .value("icon", TestValue::text("fixtures/amenity.dds"))
                .value("requiredEntity", TestValue::Reference(REQUIRED_ENTITY)),
        )
        .record(record(REQUIRED_ENTITY, ENTITY, "outside/entity.xml"))
        .record(record(RADAR_CONTACT, RADAR_TYPE, "outside/contact.xml"))
}

fn type_declaration() -> TestStruct {
    TestStruct::new(TYPE_DECL)
        .field("name", TestType::String)
        .field("classification", TestType::Locale)
        .field("facingMode", TestType::Enum(FACING.to_owned()))
        .field("minimumDisplaySize", TestType::Single)
        .field("rotationSpeed", TestType::Single)
        .field("selectable", TestType::Bool)
        .field("fadeBehindParent", TestType::Bool)
        .field("onParentSurface", TestType::Bool)
        .field("rotateNearbyObjectiveMarkers", TestType::Bool)
        .field("spawnNavPoints", TestType::Bool)
        .field("ignoreAsRoutingObstacle", TestType::Bool)
        .field("showAsNeighbor", TestType::Bool)
        .field("innerCulling", TestType::Bool)
        .field("showInMapSelectList", TestType::Bool)
        .field("markerConfig", reference("Marker_Configuration"))
        .field("subNavPointClass", reference("EntityClassDefinition"))
        .field("validQuantumTravelDestination", TestType::Bool)
        .field("geometry", class("GlobalResourceGeometry"))
        .field("material", class("GlobalResourceMaterial"))
}

fn preset_declaration() -> TestStruct {
    TestStruct::new(PRESET_DECL)
        .field("type", reference(TYPE_DECL))
        .field("visibilitySettings", class(VISIBILITY))
        .field("displaySettings", class(DISPLAY))
        .field("quantumTravelData", pointer(QT))
        .field("locationParams", class(LOCATION))
        .field("radarSettings", class(RADAR))
}

fn visibility() -> TestStruct {
    TestStruct::new(VISIBILITY)
        .field("hideInStarmap", TestType::Bool)
        .field("hideInWorld", TestType::Bool)
        .field("hideWhenInAdoptionRadius", TestType::Bool)
        .field("blockTravel", TestType::Bool)
        .field("onlyShowWhenParentSelected", TestType::Bool)
}

fn display() -> TestStruct {
    TestStruct::new(DISPLAY)
        .field("description", TestType::Locale)
        .field("navIcon", TestType::Enum(ICON.to_owned()))
        .field("size", TestType::Double)
        .field("minimumDisplaySize", TestType::Single)
        .field("useHoloMaterial", TestType::Bool)
        .field("starMapGeomPath", TestType::String)
        .field("starMapMaterialPath", TestType::String)
        .field("starMapShapePath", TestType::String)
        .field("locationMedicalImagePath", TestType::String)
        .field("assetManagerLocationParams", class(ASSET))
}

fn quantum_travel() -> TestStruct {
    TestStruct::new(QT)
        .field("obstructionRadius", TestType::Single)
        .field("arrivalRadius", TestType::Single)
        .field("arrivalPointDetectionOffset", TestType::Single)
        .field("adoptionRadius", TestType::Single)
        .field("subPointRadiusMultiplier", TestType::Single)
}

fn location() -> TestStruct {
    TestStruct::new(LOCATION)
        .field("setEntityLocationOnEnter", TestType::Bool)
        .field("exposeForPlayerCreatedMissions", TestType::Bool)
        .field("excludeFromLevelLoad", TestType::Bool)
}

fn object_type() -> TestRecord {
    record(TYPE, TYPE_DECL, "outside/type.xml")
        .value("name", TestValue::text("FixtureSuffix_ValidQT"))
        .value("classification", TestValue::text("@shared"))
        .value("facingMode", TestValue::text("FixtureFacing"))
        .value("spawnNavPoints", TestValue::Bool(true))
        .value("showInMapSelectList", TestValue::Bool(false))
        .value("validQuantumTravelDestination", TestValue::Bool(false))
        .value("markerConfig", TestValue::Reference(MISSING))
        .value(
            "geometry",
            TestValue::Struct(
                TestInstance::new("GlobalResourceGeometry")
                    .value("path", TestValue::text("fixtures/type.cgf")),
            ),
        )
}

fn preset() -> TestRecord {
    record(PRESET, PRESET_DECL, "selected/preset/first.xml")
        .value("type", TestValue::Reference(TYPE))
        .value(
            "visibilitySettings",
            TestValue::Struct(
                TestInstance::new(VISIBILITY)
                    .value("hideInWorld", TestValue::Bool(true))
                    .value("blockTravel", TestValue::Bool(true)),
            ),
        )
        .value("displaySettings", display_value())
        .value(
            "quantumTravelData",
            TestValue::Struct(
                TestInstance::new(QT)
                    .value("arrivalRadius", TestValue::Single(3.0))
                    .value("obstructionRadius", TestValue::Single(7.0))
                    .value("adoptionRadius", TestValue::Single(-0.0)),
            ),
        )
        .value(
            "locationParams",
            TestValue::Struct(
                TestInstance::new(LOCATION)
                    .value("exposeForPlayerCreatedMissions", TestValue::Bool(true))
                    .value("excludeFromLevelLoad", TestValue::Bool(false)),
            ),
        )
        .value("radarSettings", radar_value())
}

fn display_value() -> TestValue {
    TestValue::Struct(
        TestInstance::new(DISPLAY)
            .value("description", TestValue::text("@shared"))
            .value("navIcon", TestValue::text(FUTURE_ICON))
            .value(
                "size",
                TestValue::Double(f64::from_bits(0x7ff8_0000_0000_1067)),
            )
            .value("minimumDisplaySize", TestValue::Single(-0.0))
            .value("starMapGeomPath", TestValue::text("fixtures/display.cgf"))
            .value(
                "assetManagerLocationParams",
                TestValue::Struct(
                    TestInstance::new(ASSET)
                        .value("previewImagePath", TestValue::text("fixtures/preview.dds")),
                ),
            ),
    )
}

fn radar_value() -> TestValue {
    TestValue::Struct(
        TestInstance::new(RADAR)
            .value("isScannable", TestValue::Bool(true))
            .value(
                "radarProperties",
                TestValue::Struct(
                    TestInstance::new(RADAR_PROPERTIES)
                        .value("contactType", TestValue::Reference(RADAR_CONTACT))
                        .value(
                            "fixtureFutureRadarBits",
                            TestValue::Single(f32::from_bits(0x7fc0_1067)),
                        ),
                ),
            ),
    )
}

fn class(target: &str) -> TestType {
    TestType::Class(target.to_owned())
}

fn pointer(target: &str) -> TestType {
    TestType::StrongPointer(target.to_owned())
}

fn reference(target: &str) -> TestType {
    TestType::ReferenceTo(target.to_owned())
}

fn record(guid: u128, kind: &str, path: &str) -> TestRecord {
    TestRecord::new(guid, kind, format!("{kind}.Fixture{guid}"), path)
}
