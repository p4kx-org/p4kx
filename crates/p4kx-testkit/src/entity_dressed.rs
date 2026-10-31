// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::entity_armour::{armour_database, armour_files, armour_male_skeleton};
use crate::entity_head::{
    MALE_HEAD_FOLDER, MALE_RIG, head_files, head_logic, head_part, head_skin_on, skin_entries,
};
use crate::{
    TestBone, TestEntity, TestEntry, TestInstance, TestMtl, TestMtlMaterial, TestRecord,
    TestSkeleton, TestStruct, TestType, TestValue,
};
/// The GUID of the hair Entity: a skin for the male body, drawn with [`DRESSED_HAIR_MATERIAL`].
pub const DRESSED_HAIR: u128 = 0x51;
/// The GUID of the eyes Entity, whose male skin is nested in a sub-node tagged `Protos_head`.
pub const DRESSED_EYES: u128 = 0x52;
/// The GUID of an Entity that names no geometry, such as the brows of a bald character.
pub const DRESSED_BROWS: u128 = 0x53;
/// The GUID of a hair Entity whose skin the Archive lacks.
pub const DRESSED_GONE_HAIR: u128 = 0x54;
/// The GUID of the material the lookup table maps to [`DRESSED_HEAD_MATERIAL_FILE`].
pub const DRESSED_HEAD_MATERIAL: u128 = 0x61;
/// The GUID of a material that no lookup table lists.
pub const DRESSED_UNKNOWN_MATERIAL: u128 = 0x62;
/// The facial bone of the head of [`dressed_jaw_entries`], below `head`, which the hair skin lacks.
pub const DRESSED_JAW_BONE: &str = "jaw";
/// The index of [`DRESSED_JAW_BONE`] in the skeleton of the head skin.
pub const DRESSED_JAW_INDEX: u16 = 3;
/// The material library of the head that [`DRESSED_HEAD_MATERIAL`] stands for: three
/// `shader_head` skins with a diffuse map and the average colour `0.5, 0.25, 0.125`.
pub const DRESSED_HEAD_MATERIAL_FILE: &str =
    "Objects/Characters/Human/heads/male/npc/male17/male17_t1_head_material.mtl";
/// The diffuse map of [`DRESSED_HEAD_MATERIAL_FILE`], as a material file names it.
pub const DRESSED_HEAD_DIFFUSE: &str =
    "Objects/Characters/Human/heads/male/npc/male17/textures/male17_t1_head_diff.tif";
/// The material library of the hair: three `hair_test_m` materials of the shader `HairPBR` that
/// take a dye and have an opacity map.
pub const DRESSED_HAIR_MATERIAL: &str =
    "Objects/Characters/Human/heads/shared/hair_v2/sc/male/m_hair_test.mtl";
/// The opacity map of [`DRESSED_HAIR_MATERIAL`], as a material file names it.
pub const DRESSED_HAIR_OPACITY: &str =
    "Objects/Characters/Human/heads/shared/hair_v2/textures/hair_test_opac.tif";
/// The material library of the eyes: three `shader_eyeInner` materials of the shader `Eye`.
pub const DRESSED_EYES_MATERIAL: &str =
    "Objects/Characters/Human/heads/shared/eyes/eyes_test_material.mtl";

const HAIR_SKIN: &str = "Objects/Characters/Human/heads/shared/hair_v2/sc/male/m_hair_test";

fn windows(path: &str) -> String {
    format!("Data\\{}", path.replace('/', "\\"))
}

fn lookup_structs() -> Vec<TestStruct> {
    let class = |name: &str| TestType::Class(name.to_owned());
    vec![
        TestStruct::new("SCharacterGenerationParams").field(
            "materialLookupTable",
            class("SCustomizableMaterialLookupTable"),
        ),
        TestStruct::new("SCustomizableMaterialLookupTable")
            .array("materials", class("SCustomizableMaterialEntry")),
        TestStruct::new("SCustomizableMaterialEntry")
            .field("filePath", TestType::String)
            .field("guid", TestType::Guid),
    ]
}

fn lookup_record() -> TestRecord {
    let entry = TestValue::Struct(
        TestInstance::new("SCustomizableMaterialEntry")
            .value("filePath", TestValue::text(DRESSED_HEAD_MATERIAL_FILE))
            .value("guid", TestValue::Guid(DRESSED_HEAD_MATERIAL)),
    );
    let table = TestInstance::new("SCustomizableMaterialLookupTable")
        .value("materials", TestValue::Array(vec![entry]));
    TestRecord::new(
        0x70,
        "SCharacterGenerationParams",
        "SCharacterGenerationParams.TestParams",
        "libs/foundry/records/gamemode/testcharactergenerationparams.xml",
    )
    .value("materialLookupTable", TestValue::Struct(table))
}

fn database() -> Vec<u8> {
    let hair = TestEntity::new(DRESSED_HAIR, "TEST_Hair", "entities/scitem/test_hair.xml")
        .geometry_variant(
            "Male",
            format!("{HAIR_SKIN}.skin"),
            Some(DRESSED_HAIR_MATERIAL),
        );
    let eyes = TestEntity::new(DRESSED_EYES, "TEST_Eyes", "entities/scitem/test_eyes.xml")
        .geometry_variant(
            "Protos_head>Male",
            format!("{MALE_HEAD_FOLDER}/{MALE_RIG}_eyes.skin"),
            Some(DRESSED_EYES_MATERIAL),
        );
    let brows = TestEntity::new(
        DRESSED_BROWS,
        "TEST_Brows",
        "entities/scitem/test_brows.xml",
    );
    let gone = TestEntity::new(
        DRESSED_GONE_HAIR,
        "TEST_GoneHair",
        "entities/scitem/test_gone_hair.xml",
    )
    .geometry_variant("Male", "Objects/Test/missing_hair.skin", None);
    let builder = lookup_structs()
        .into_iter()
        .fold(armour_database(), crate::DataForgeBuilder::structure);
    [hair, eyes, brows, gone]
        .iter()
        .fold(builder, |builder, entity| builder.record(entity.record()))
        .record(lookup_record())
        .build()
}

fn material_file(path: &str, materials: &[TestMtlMaterial]) -> TestEntry {
    TestEntry::stored(windows(path), TestMtl::multi(materials).cryxml())
}

/// Returns the Entries of a synthetic Archive that holds the head of [`head_entries`] with its
/// teeth, and what a character wears on it:
///
/// - the eyes skin (a cube, with the rig `rig` beside it), the male hair skin and their material
///   libraries ([`DRESSED_EYES_MATERIAL`], [`DRESSED_HAIR_MATERIAL`]) and the material library of
///   the head ([`DRESSED_HEAD_MATERIAL_FILE`]);
/// - the Entities `TEST_Hair`, `TEST_Eyes`, `TEST_Brows` and `TEST_GoneHair` named by the GUIDs
///   above, and a Character generation record whose lookup table lists
///   [`DRESSED_HEAD_MATERIAL`] only.
///
/// [`head_entries`](crate::head_entries) describes the head and the bodies.
#[must_use]
pub fn dressed_entries(rig: Option<Vec<u8>>) -> Vec<TestEntry> {
    let mut entries = vec![TestEntry::zstd("Data\\Game2.dcb", database())];
    entries.extend(armour_files());
    entries.extend(head_files(rig.clone()));
    entries.extend(skin_entries(&head_part("eyes"), rig));
    entries.extend(skin_entries(&windows(HAIR_SKIN), None));
    let three = |material: &TestMtlMaterial| [material.clone(), material.clone(), material.clone()];
    entries.push(material_file(
        DRESSED_HEAD_MATERIAL_FILE,
        &three(
            &TestMtlMaterial::new("shader_head")
                .shader("HumanSkin_V2")
                .diffuse([1.0; 3])
                .texture("TexSlot1", DRESSED_HEAD_DIFFUSE)
                .public_param("SourceAverageColor", "0.5,0.25,0.125"),
        ),
    ));
    entries.push(material_file(
        DRESSED_HAIR_MATERIAL,
        &three(
            &TestMtlMaterial::new("hair_test_m")
                .shader("HairPBR")
                .texture("TexSlot1", DRESSED_HAIR_OPACITY)
                .public_param("DyeAmount", "0")
                .public_param("BaseMelanin", "0.5"),
        ),
    ));
    entries.push(material_file(
        DRESSED_EYES_MATERIAL,
        &three(
            &TestMtlMaterial::new("shader_eyeInner")
                .shader("Eye")
                .public_param("IrisColor", "0.25,0.25,0.25"),
        ),
    ));
    entries
}

/// Returns the Entries of [`dressed_entries`] with a head skin whose every vertex is weighted to
/// [`DRESSED_JAW_BONE`], a bone 0.1 m above `head` that the male body and the hair skin lack.
#[must_use]
pub fn dressed_jaw_entries(rig: Option<Vec<u8>>) -> Vec<TestEntry> {
    let stem = head_part("head");
    let replaced = [format!("{stem}.skin"), format!("{stem}.skinm")];
    let mut bones = armour_male_skeleton().bones().to_vec();
    bones.push(TestBone {
        name: DRESSED_JAW_BONE.to_owned(),
        parent: Some(2),
        rotation: [0.0, 0.0, 0.0, 1.0],
        translation: [0.0, 0.0, 0.1],
    });
    let model = head_skin_on(&TestSkeleton::new(bones), DRESSED_JAW_INDEX, &head_logic());
    let mut entries: Vec<TestEntry> = dressed_entries(rig)
        .into_iter()
        .filter(|entry| {
            !replaced
                .iter()
                .any(|path| entry.path().eq_ignore_ascii_case(path))
        })
        .collect();
    entries.push(TestEntry::stored(&replaced[0], model.cgf()));
    entries.push(TestEntry::stored(&replaced[1], model.cgfm()));
    entries
}
