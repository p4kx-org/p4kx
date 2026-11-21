// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! A synthetic Archive whose ships and items cover every playability value and `null`.

use std::error::Error;
use std::path::{Path, PathBuf};

use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, GlobalIniBuilder, TestEntry, TestInstance, TestRecord,
    TestStruct, TestType, TestValue,
};

mod specs;

use specs::{
    CREWED_MANIFEST, EMPTY_MANIFEST, ITEMS, ItemSpec, OTHER_TAG, SHIPS, ShipSpec, UNMANNED_TAG,
};

fn guid(kind: u128, index: usize) -> u128 {
    (kind << 120) | 0x4000_8000_0000_0000_0000 | (u128::try_from(index).unwrap_or(0) + 1)
}

fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}

fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}

fn component(name: &str) -> TestStruct {
    TestStruct::new(name).parent("DataForgeComponentParams")
}

fn structures(builder: DataForgeBuilder, corrupt: Corrupt) -> DataForgeBuilder {
    let tag = if corrupt == Corrupt::Tags {
        unreadable("Tag")
    } else {
        TestStruct::new("Tag").field("tagName", TestType::String)
    };
    let manifest = if corrupt == Corrupt::Manifests {
        unreadable("CrewManifest")
    } else {
        TestStruct::new("CrewManifest").array("crew", class("CrewEntry"))
    };
    [
        tag,
        TestStruct::new("CrewEntry").field("name", TestType::String),
        manifest,
        TestStruct::new("SCItemLocalization").field("Name", TestType::Locale),
        TestStruct::new("SItemDefinition").field("Localization", class("SCItemLocalization")),
        TestStruct::new("SAIEngineeringOptions")
            .field("powerPlantsCanExplodeOnDeath", TestType::Bool),
        TestStruct::new("StaticEntityClassData"),
        TestStruct::new("EAEntityDataParams")
            .parent("StaticEntityClassData")
            .field("inclusionMode", TestType::Enum("EInclusionMode".to_owned())),
        TestStruct::new("EntityClassDefinition")
            .field("Category", TestType::String)
            .array("Components", pointer("DataForgeComponentParams"))
            .array("tags", TestType::Reference)
            .array("StaticEntityClassData", pointer("StaticEntityClassData")),
        TestStruct::new("DataForgeComponentParams"),
        component("SAttachableComponentParams").field("AttachDef", class("SItemDefinition")),
        component("SCItemPurchasableParams").field("displayName", TestType::Locale),
        component("VehicleComponentParams")
            .field("criticalPartExplosionChance", TestType::Single)
            .field("aiEngineeringOptions", class("SAIEngineeringOptions")),
        component("SEntityComponentCrewManifestParams").field("crewManifest", TestType::Reference),
    ]
    .into_iter()
    .fold(builder, DataForgeBuilder::structure)
}

fn entity(
    guid: u128,
    class: &str,
    path: &str,
    components: Vec<TestValue>,
    tags: &[u128],
    inclusion: Option<&str>,
) -> TestRecord {
    let data = inclusion.map_or_else(Vec::new, |mode| {
        vec![TestValue::Struct(
            TestInstance::new("EAEntityDataParams").value("inclusionMode", TestValue::text(mode)),
        )]
    });
    TestRecord::new(
        guid,
        "EntityClassDefinition",
        format!("EntityClassDefinition.{class}"),
        format!("libs/foundry/records/entities/{path}.xml"),
    )
    .value("Category", TestValue::text("Default"))
    .value("Components", TestValue::Array(components))
    .value(
        "tags",
        TestValue::Array(tags.iter().copied().map(TestValue::Reference).collect()),
    )
    .value("StaticEntityClassData", TestValue::Array(data))
}

fn ship(index: usize, spec: &ShipSpec) -> TestRecord {
    let mut components = vec![TestValue::Struct(
        TestInstance::new("VehicleComponentParams")
            .value(
                "criticalPartExplosionChance",
                TestValue::Single(spec.explosion),
            )
            .value(
                "aiEngineeringOptions",
                TestValue::Struct(TestInstance::new("SAIEngineeringOptions").value(
                    "powerPlantsCanExplodeOnDeath",
                    TestValue::Bool(spec.hull_explodes),
                )),
            ),
    )];
    if let Some(manifest) = spec.manifest {
        components.push(TestValue::Struct(
            TestInstance::new("SEntityComponentCrewManifestParams")
                .value("crewManifest", TestValue::Reference(manifest)),
        ));
    }
    entity(
        guid(0x90, index),
        spec.class,
        &format!("{}/{}", spec.folder, spec.class.to_ascii_lowercase()),
        components,
        spec.tags,
        spec.inclusion,
    )
    .tag(spec.record_tag)
}

fn item(index: usize, spec: &ItemSpec) -> TestRecord {
    let mut components = Vec::new();
    if let Some(name) = spec.name {
        components.push(TestValue::Struct(
            TestInstance::new("SAttachableComponentParams").value(
                "AttachDef",
                TestValue::Struct(
                    TestInstance::new("SItemDefinition").value(
                        "Localization",
                        TestValue::Struct(
                            TestInstance::new("SCItemLocalization")
                                .value("Name", TestValue::text(name)),
                        ),
                    ),
                ),
            ),
        ));
    }
    if let Some(shop) = spec.shop {
        components.push(TestValue::Struct(
            TestInstance::new("SCItemPurchasableParams")
                .value("displayName", TestValue::text(shop)),
        ));
    }
    entity(
        guid(0x30, index),
        spec.class,
        spec.path,
        components,
        &[],
        None,
    )
}

fn manifest(guid: u128, name: &str, crew: usize, corrupt: Corrupt) -> TestRecord {
    let record = TestRecord::new(
        guid,
        "CrewManifest",
        format!("CrewManifest.{name}"),
        format!("libs/foundry/records/crewmanifest/{name}.xml"),
    );
    if corrupt == Corrupt::Manifests {
        return record;
    }
    record.value(
        "crew",
        TestValue::Array(
            (0..crew)
                .map(|_| {
                    TestValue::Struct(
                        TestInstance::new("CrewEntry").value("name", TestValue::text("pilot")),
                    )
                })
                .collect(),
        ),
    )
}

fn tag(guid: u128, name: &str, corrupt: Corrupt) -> TestRecord {
    let record = TestRecord::new(
        guid,
        "Tag",
        format!("Tag.{name}"),
        "libs/foundry/records/tagdatabase/tagdatabase.tagdatabase",
    );
    if corrupt == Corrupt::Tags {
        return record;
    }
    record.value("tagName", TestValue::text(name))
}

/// Which Records of the fixture cannot be read.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Corrupt {
    /// Every Record can be read.
    Nothing,
    /// The crew manifests cannot be read.
    Manifests,
    /// The tags cannot be read.
    Tags,
}

fn unreadable(name: &str) -> TestStruct {
    TestStruct::new(name)
        .field("a", TestType::Int8)
        .declared_size(3)
}

fn database(corrupt: Corrupt) -> DataForgeBuilder {
    let builder = structures(
        DataForgeBuilder::new().version(8).enumeration(
            "EInclusionMode",
            ["ReadyToInclude", "DoNotInclude", "SomethingNew"],
        ),
        corrupt,
    )
    .record(manifest(CREWED_MANIFEST, "crewed", 2, corrupt))
    .record(manifest(EMPTY_MANIFEST, "crewless", 0, corrupt))
    .record(tag(UNMANNED_TAG, "Unmanned", corrupt))
    .record(tag(OTHER_TAG, "Ship", corrupt));
    let builder = SHIPS
        .iter()
        .enumerate()
        .fold(builder, |builder, (index, spec)| {
            builder.record(ship(index, spec))
        });
    ITEMS
        .iter()
        .enumerate()
        .fold(builder, |builder, (index, spec)| {
            builder.record(item(index, spec))
        })
}

fn english() -> TestEntry {
    TestEntry::zstd(
        "Data\\Localization\\english\\global.ini",
        GlobalIniBuilder::new()
            .text("item_NameShop", "Shop item")
            .text("item_NameStash", "Stash")
            .text("item_NameNamed", "Named item")
            .build(),
    )
}

/// Writes the fixture Archive whose `corrupt` Records cannot be read into `dir` and returns its
/// path.
pub(crate) fn write_archive_with(dir: &Path, corrupt: Corrupt) -> Result<PathBuf, Box<dyn Error>> {
    write_archive(dir, &database(corrupt), true)
}

/// Writes a fixture whose empty crew manifest has a second, crewed physical Record.
pub(crate) fn write_archive_with_crew_alias(
    dir: &Path,
    encrypted: bool,
) -> Result<PathBuf, Box<dyn Error>> {
    let database = database(Corrupt::Nothing).record(manifest(
        EMPTY_MANIFEST,
        "crew_alias",
        1,
        Corrupt::Nothing,
    ));
    write_archive(dir, &database, encrypted)
}

fn write_archive(
    dir: &Path,
    database: &DataForgeBuilder,
    encrypted: bool,
) -> Result<PathBuf, Box<dyn Error>> {
    let path = dir.join("Data.p4k");
    let dataforge = TestEntry::zstd("Data\\Game2.dcb", database.build());
    let dataforge = if encrypted {
        dataforge.encrypted()
    } else {
        dataforge
    };
    ArchiveBuilder::new()
        .entry(dataforge)
        .entry(english())
        .write_to(&path)?;
    Ok(path)
}
