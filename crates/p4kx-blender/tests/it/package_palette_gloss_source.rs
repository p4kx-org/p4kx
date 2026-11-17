// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_testkit::{
    DataForgeBuilder, SplitDds, TestEntity, TestEntry, TestLoadout, TestLoadoutEntry, TestModel,
    TestMtl, TestMtlMaterial, TestTextureFormat, entity_schema, tint_palette_record,
};

pub(super) const ROOT_CLASS: &str = "TEST_Gloss_0_Half";
pub(super) const FINISH_PALETTE_BYTE: u8 = 184;
pub(super) const PALETTE_BYTES: [u8; 5] = [0, 1, 128, 255, FINISH_PALETTE_BYTE];
pub(super) const NORMAL_ALPHA: u8 = 196;
pub(super) const TEXTURE_PATH: &str = "Objects/Test/palette_sample_ddna.dds";
pub(super) const EXPLICIT_GLOSS_PATH: &str = "Objects/Test/palette_explicit_gloss.dds";
const ALPHA_FREE_NORMAL: &str = "Objects/Test/palette_rgb_ddna.dds";
const EXPLICIT_GLOSS_GREEN: u8 = 163;

pub(super) struct ExplicitGlossCase {
    pub(super) name: &'static str,
    pub(super) normal: Option<&'static str>,
    pub(super) green: u8,
}

pub(super) const EXPLICIT_GLOSS_CASES: [ExplicitGlossCase; 3] = [
    ExplicitGlossCase {
        name: "ExplicitGloss",
        normal: None,
        green: EXPLICIT_GLOSS_GREEN,
    },
    ExplicitGlossCase {
        name: "ExplicitGlossAlphaFree",
        normal: Some(ALPHA_FREE_NORMAL),
        green: EXPLICIT_GLOSS_GREEN,
    },
    ExplicitGlossCase {
        name: "ExplicitGlossPackedDdna",
        normal: Some(TEXTURE_PATH),
        green: u8::MAX - NORMAL_ALPHA,
    },
];
const MODEL: &str = "Objects/Test/palette_sample.cgf";
const SAMPLED_LAYER: &str = "Objects/Test/palette_sample.mtl";
const CONSTANT_LAYER: &str = "Objects/Test/palette_constant.mtl";
const FIRST_PALETTE_GUID: u128 = 0x9500;
const FIRST_ENTITY_GUID: u128 = 0x9600;

const MULTIPLIERS: [Multiplier; 2] = [
    Multiplier {
        label: "Half",
        token: Some("0.5"),
        value: 0.5,
    },
    Multiplier {
        label: "Zero",
        token: Some("0"),
        value: 0.0,
    },
];

#[derive(Clone, Copy)]
pub(super) struct Multiplier {
    pub(super) label: &'static str,
    pub(super) token: Option<&'static str>,
    pub(super) value: f32,
}

pub(super) fn palette_cases() -> impl Iterator<Item = (u8, Multiplier)> {
    let neutral = Multiplier {
        label: "Neutral",
        token: None,
        value: 1.0,
    };
    PALETTE_BYTES
        .into_iter()
        .flat_map(|byte| {
            MULTIPLIERS
                .into_iter()
                .map(move |multiplier| (byte, multiplier))
        })
        .chain(std::iter::once((FINISH_PALETTE_BYTE, neutral)))
}

pub(super) fn material_name(byte: u8, multiplier: Multiplier, sampled: bool) -> String {
    let input = if sampled { "Sample" } else { "Constant" };
    format!("Gloss_{byte}_{}_{input}", multiplier.label)
}

pub(super) fn palette_name(byte: u8) -> String {
    format!("p_gloss_{byte}")
}

pub(super) fn entries() -> Result<Vec<TestEntry>, Box<dyn Error>> {
    let model = TestModel::cube();
    let mut entries = vec![
        TestEntry::stored(format!("Data/{MODEL}"), model.cgf()),
        TestEntry::stored("Data/Objects/Test/palette_sample.cgfm", model.cgfm()),
        TestEntry::stored(
            format!("Data/{TEXTURE_PATH}"),
            texture(
                TestTextureFormat::LegacyBgra8,
                [255, 128, 128, NORMAL_ALPHA],
            )?,
        ),
        TestEntry::zstd(
            format!("Data/{EXPLICIT_GLOSS_PATH}"),
            texture(TestTextureFormat::LegacyBgra8, [224, 96, 32, 17])?,
        )
        .encrypted(),
        TestEntry::stored(
            format!("Data/{ALPHA_FREE_NORMAL}"),
            texture(TestTextureFormat::LegacyBgr8, [255, 128, 128])?,
        ),
        TestEntry::stored(format!("Data/{SAMPLED_LAYER}"), layer(true)),
        TestEntry::stored(format!("Data/{CONSTANT_LAYER}"), layer(false)),
    ];
    let database = database(&mut entries);
    entries.push(TestEntry::zstd("Data/Game2.dcb", database).encrypted());
    Ok(entries)
}

fn database(entries: &mut Vec<TestEntry>) -> Vec<u8> {
    let mut database = entity_schema(DataForgeBuilder::new());
    let mut root = None;
    let mut children = Vec::new();
    let mut ids = FIRST_ENTITY_GUID..;
    for ((byte, multiplier), id) in palette_cases().zip(ids.by_ref()) {
        let class = format!("TEST_Gloss_{byte}_{}", multiplier.label);
        let material = format!("Objects/Test/gloss_{byte}_{}", multiplier.label);
        entries.push(TestEntry::stored(
            format!("Data/{material}.mtl"),
            library(byte, multiplier),
        ));
        let entity = TestEntity::new(id, &class, format!("entities/scitem/{class}.xml"))
            .geometry(MODEL, Some(&material))
            .palette(FIRST_PALETTE_GUID + u128::from(byte), "");
        if class == ROOT_CLASS {
            root = Some(entity);
        } else {
            children.push(TestLoadoutEntry::class(class.as_str(), class.as_str()));
            database = database.record(entity.record());
        }
    }
    for (case, id) in EXPLICIT_GLOSS_CASES.iter().zip(ids) {
        let class = format!("TEST_{}", case.name);
        let library = format!("Objects/Test/{}", case.name);
        let mut material = TestMtlMaterial::new(case.name)
            .shader("HardSurface")
            .texture("TexSlot6", EXPLICIT_GLOSS_PATH);
        if let Some(normal) = case.normal {
            material = material.texture("TexSlot2", normal);
        }
        entries.push(TestEntry::stored(
            format!("Data/{library}.mtl"),
            TestMtl::multi(&[TestMtlMaterial::new("unused"), material]).cryxml(),
        ));
        children.push(TestLoadoutEntry::class(class.as_str(), class.as_str()));
        database = database.record(
            TestEntity::new(id, &class, format!("entities/scitem/{class}.xml"))
                .geometry(MODEL, Some(&library))
                .record(),
        );
    }
    for byte in PALETTE_BYTES {
        database = database.record(tint_palette_record(
            FIRST_PALETTE_GUID + u128::from(byte),
            &palette_name(byte),
            [[255, 0, 0]; 3],
            [255; 3],
            [byte; 3],
        ));
    }
    if let Some(root) = root {
        database = database.record(root.loadout(TestLoadout::Manual(children)).record());
    }
    database.build()
}

fn library(byte: u8, multiplier: Multiplier) -> Vec<u8> {
    let make = |sampled| {
        let path = if sampled {
            SAMPLED_LAYER
        } else {
            CONSTANT_LAYER
        };
        let mut attributes = vec![("Name", "Paint"), ("PaletteTint", "1"), ("Path", path)];
        if let Some(token) = multiplier.token {
            attributes.push(("GlossMult", token));
        }
        let material = TestMtlMaterial::new(material_name(byte, multiplier, sampled))
            .shader("HardSurface")
            .layer_with(&attributes);
        if sampled {
            material.texture("TexSlot2", TEXTURE_PATH)
        } else {
            material
        }
    };
    TestMtl::multi(&[TestMtlMaterial::new("unused"), make(true), make(false)]).cryxml()
}

fn layer(sampled: bool) -> Vec<u8> {
    let layer = TestMtlMaterial::new("layer")
        .shader("Layer")
        .diffuse([1.0; 3]);
    let layer = if sampled {
        layer.texture("TexSlot2", TEXTURE_PATH)
    } else {
        layer
    };
    TestMtl::single(&layer).cryxml()
}

fn texture<const CHANNELS: usize>(
    format: TestTextureFormat,
    pixel_bgr: [u8; CHANNELS],
) -> Result<Vec<u8>, Box<dyn Error>> {
    let dds = SplitDds::new(format, 2, 2).mips(1);
    let mut bytes = dds.merged_bytes();
    let sample = bytes
        .get_mut(dds.header_len()..)
        .ok_or("original DDS pixels")?;
    let (pixels, []) = sample.as_chunks_mut::<CHANNELS>() else {
        return Err("the original DDS has whole texels".into());
    };
    pixels.fill(pixel_bgr);
    Ok(bytes)
}
