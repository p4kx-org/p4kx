// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, Language};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestContainer, TestEntity, TestEntry, TestRecord, TestStruct,
    TestType, TestValue, entity_schema,
};

use super::{TestResult, commodities};

pub(super) const ROOT: &str = r"Data\ObjectContainers\PU\system\fixture\fixturesystem.socpak";
pub(super) const OFFLINE_SHOP: &str = r"Data\Scripts\ShopInventories\Inv_Fixture.json";
pub(super) const FLARE_LIBRARY: &str = r"Data\Libs\Flares\fixture.xml";
pub(super) const SUBSUMPTION_FIXTURE: &str = "Data/Libs/Subsumption/Events/synthetic.xml";
const TILE: &str = r"Data\ObjectContainers\PU\fixture\tile.socpak";
const ROOT_XML: &[u8] = br#"<ObjectContainer version="14" gridRadius="2"><PhysicsGrid/><ChildObjectContainers>
    <Child external="true" name="Data\ObjectContainers\PU\fixture\tile.socpak" pos="0,0,0" rot="1,0,0,0" locationType="3" class="FixtureLocation"/>
    <Child external="true" name="Data\ObjectContainers\PU\fixture\missing.socpak" pos="1,0,0" rot="1,0,0,0"/>
</ChildObjectContainers></ObjectContainer>"#;

pub(super) fn open() -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("sdk_routes.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data\\Game2.dcb",
            fixture_database().build(),
        ))
        .entry(TestEntry::stored(
            Language::German.global_ini_path(),
            b"fixture=Fixture\n",
        ))
        .entry(packed(
            ROOT,
            "fixturesystem.xml",
            ROOT_XML,
            [TestEntry::stored(
                "entdata/loot.entxml",
                br#"<Entity objectContainer="Data/ObjectContainers/PU/fixture/loot.socpak"/>"#,
            )],
        ))
        .entry(packed(TILE, "tile.xml", b"<ObjectContainer/>", []))
        .entry(TestEntry::stored(OFFLINE_SHOP, br#"{"Inventory":[]}"#))
        .entry(TestEntry::stored(
            "Data/Libs/GameAudio/RadarAudioData.xml",
            b"<RadarAudioData><DefaultZoom min='0'/></RadarAudioData>",
        ))
        .entry(TestEntry::stored(
            FLARE_LIBRARY,
            b"<LensFlareLibrary><FlareItem Name='fixture'/></LensFlareLibrary>",
        ))
        .entry(TestEntry::stored(
            SUBSUMPTION_FIXTURE,
            b"<Subsumption><Event ID='00000000-0000-0000-0000-000000000001'/></Subsumption>",
        ))
        .write_to(&path)?;
    Ok((folder, Archive::open(path)?))
}

fn fixture_database() -> DataForgeBuilder {
    let database = entity_schema(DataForgeBuilder::new().version(8))
        .structure(TestStruct::new("AmmoParams"))
        .record(TestRecord::new(
            1,
            "AmmoParams",
            "AmmoParams.FixtureBallistic",
            "libs/foundry/records/ammoparams/fixture_ballistic.xml",
        ))
        .structure(TestStruct::new("WeaponProceduralRecoilConfigDef"))
        .record(TestRecord::new(
            2,
            "WeaponProceduralRecoilConfigDef",
            "WeaponProceduralRecoilConfigDef.Fixture",
            "libs/foundry/records/fixture/recoil.xml",
        ))
        .structure(TestStruct::new("LocationEntityDeclaration"))
        .record(TestRecord::new(
            3,
            "LocationEntityDeclaration",
            "LocationEntityDeclaration.Fixture",
            "libs/foundry/records/fixture/location_entity.xml",
        ))
        .structure(TestStruct::new("GlobalCargoLoadingParams"))
        .record(TestRecord::new(
            4,
            "GlobalCargoLoadingParams",
            "GlobalCargoLoadingParams.Fixture",
            "libs/foundry/records/fixture/cargo_loading.xml",
        ))
        .structure(TestStruct::new("GlobalShopCommodityParams"))
        .record(TestRecord::new(
            5,
            "GlobalShopCommodityParams",
            "GlobalShopCommodityParams.Fixture",
            "libs/foundry/records/fixture/shop_commodity.xml",
        ))
        .structure(TestStruct::new("SCItemManufacturer").field("Code", TestType::String))
        .record(
            TestRecord::new(
                6,
                "SCItemManufacturer",
                "SCItemManufacturer.FixtureMaker",
                "libs/foundry/records/scitemmanufacturer/fixture.xml",
            )
            .value("Code", TestValue::text("FIX")),
        )
        .record(
            TestEntity::new(
                7,
                "FixtureTool",
                "libs/foundry/records/entities/scitem/fixture/tool.xml",
            )
            .attach_type("Misc")
            .record(),
        )
        .record(commodities::fixture_record());
    with_tag_declarations(with_starmap(with_faction_identity(database)))
}

fn with_tag_declarations(database: DataForgeBuilder) -> DataForgeBuilder {
    database
        .structure(
            TestStruct::new("Tag")
                .field("tagName", TestType::String)
                .field("legacyGUID", TestType::UInt32)
                .array("children", TestType::ReferenceTo("Tag".to_owned())),
        )
        .structure(
            TestStruct::new("TagDatabase").array("tags", TestType::ReferenceTo("Tag".to_owned())),
        )
        .record(
            TestRecord::new(11, "Tag", "Tag.Fixture", "fixtures/tag.xml")
                .value("tagName", TestValue::text("Fixture"))
                .value("legacyGUID", TestValue::UInt32(0))
                .value("children", TestValue::Array(Vec::new())),
        )
        .record(
            TestRecord::new(
                12,
                "TagDatabase",
                "TagDatabase.Fixture",
                "fixtures/tag_database.xml",
            )
            .value("tags", TestValue::Array(vec![TestValue::Reference(11)])),
        )
}

fn with_starmap(database: DataForgeBuilder) -> DataForgeBuilder {
    database
        .structure(TestStruct::new("StarMapObject"))
        .record(TestRecord::new(
            10,
            "StarMapObject",
            "StarMapObject.Fixture",
            "libs/foundry/records/starmap/pu/system/fixture/loot.xml",
        ))
}

fn with_faction_identity(database: DataForgeBuilder) -> DataForgeBuilder {
    database
        .structure(TestStruct::new("FactionIdentity"))
        .record(TestRecord::new(
            9,
            "FactionIdentity",
            "FactionIdentity.Fixture",
            "libs/foundry/records/fixture/faction_identity.xml",
        ))
}

fn packed(
    path: &str,
    file: &str,
    xml: &[u8],
    members: impl IntoIterator<Item = TestEntry>,
) -> TestEntry {
    let container = TestContainer::new().entry(TestEntry::stored(file, xml));
    TestEntry::stored(
        path,
        members
            .into_iter()
            .fold(container, TestContainer::entry)
            .build(),
    )
}
