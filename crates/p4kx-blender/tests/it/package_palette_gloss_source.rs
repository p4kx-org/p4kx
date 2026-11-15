// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_testkit::{
    DataForgeBuilder, SplitDds, TestEntity, TestEntry, TestLoadout, TestLoadoutEntry, TestModel,
    TestMtl, TestMtlMaterial, TestTextureFormat, entity_schema, tint_palette_record,
};

pub(super) const ROOT_CLASS: &str = "TEST_Gloss_0_Half";
pub(super) const PALETTE_BYTES: [u8; 4] = [0, 1, 128, 255];
pub(super) const NORMAL_ALPHA: u8 = 128;
pub(super) const TEXTURE_PATH: &str = "Objects/Test/palette_sample_ddna.dds";
const MODEL: &str = "Objects/Test/palette_sample.cgf";
const SAMPLED_LAYER: &str = "Objects/Test/palette_sample.mtl";
const CONSTANT_LAYER: &str = "Objects/Test/palette_constant.mtl";
const FIRST_PALETTE_GUID: u128 = 0x9500;
const FIRST_ENTITY_GUID: u128 = 0x9600;

pub(super) const MULTIPLIERS: [Multiplier; 2] = [
    Multiplier {
        label: "Half",
        token: "0.5",
        value: 0.5,
    },
    Multiplier {
        label: "Zero",
        token: "0",
        value: 0.0,
    },
];

#[derive(Clone, Copy)]
pub(super) struct Multiplier {
    pub(super) label: &'static str,
    pub(super) token: &'static str,
    pub(super) value: f32,
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
        TestEntry::stored(format!("Data/{TEXTURE_PATH}"), texture()?),
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
    let cases = PALETTE_BYTES.into_iter().flat_map(|byte| {
        MULTIPLIERS
            .into_iter()
            .map(move |multiplier| (byte, multiplier))
    });
    for (id, (byte, multiplier)) in (FIRST_ENTITY_GUID..).zip(cases) {
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
        let material = TestMtlMaterial::new(material_name(byte, multiplier, sampled))
            .shader("HardSurface")
            .layer_with(&[
                ("Name", "Paint"),
                ("PaletteTint", "1"),
                ("Path", path),
                ("GlossMult", multiplier.token),
            ]);
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

fn texture() -> Result<Vec<u8>, Box<dyn Error>> {
    const PIXEL_BGRA: [u8; 4] = [255, 128, 128, NORMAL_ALPHA];
    let dds = SplitDds::new(TestTextureFormat::LegacyBgra8, 2, 2).mips(1);
    let mut bytes = dds.merged_bytes();
    let sample = bytes
        .get_mut(dds.header_len()..)
        .ok_or("original DDS colour and alpha")?;
    let (pixels, []) = sample.as_chunks_mut::<{ PIXEL_BGRA.len() }>() else {
        return Err("the original DDS has whole BGRA texels".into());
    };
    pixels.fill(PIXEL_BGRA);
    Ok(bytes)
}
