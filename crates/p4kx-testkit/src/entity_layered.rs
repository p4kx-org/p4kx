// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{
    CUBE_CAPS_MATERIAL, CUBE_SIDES_MATERIAL, DataForgeBuilder, SplitDds, TestElement, TestEntity,
    TestEntry, TestModel, TestMtl, TestMtlMaterial, TestSubmesh, TestTextureFormat, entity_schema,
    tint_palette_record,
};

/// The sRGB bytes of the tint colours A to C of the palette `p_layered` of `TEST_Layered`.
pub const LAYERED_TINTS: [[u8; 3]; 3] = [[255, 0, 0], [0, 255, 0], [0, 0, 255]];

/// The textures the materials of `TEST_Layered` name that the Archive does not hold:
/// `Objects/Test/layers/textures/wear_spec`.
pub const LAYERED_MISSING_TEXTURE: &str = "Objects/Test/layers/textures/wear_spec.tif";

const SIDE: u32 = 8;
const CUBE_VERTICES: usize = 24;
const CUBE_COLOUR: [u8; 4] = [10, 20, 30, 255];
const LAYER_TEXTURES: [&str; 8] = [
    "base_diff",
    "base_ddna",
    "base_detail",
    "base_blend",
    "wear_diff",
    "wear_ddna",
    "decal_diff",
    "canopy_ddna",
];

fn database() -> DataForgeBuilder {
    let ship = TestEntity::new(0x41, "TEST_Layered", "entities/spaceships/test_layered.xml")
        .geometry(
            "Objects/Test/layered_ship.cgf",
            Some("Objects/Test/layered"),
        )
        .palette(0x42, "");
    entity_schema(DataForgeBuilder::new())
        .record(ship.record())
        .record(tint_palette_record(
            0x42,
            "p_layered",
            LAYERED_TINTS,
            [255; 3],
            [51, 102, 153],
        ))
}

fn texture(name: &str) -> String {
    format!("Objects/Test/layers/textures/{name}.tif")
}

fn library() -> Vec<u8> {
    let hull = TestMtlMaterial::new("hull")
        .shader("HardSurface")
        .string_gen_mask("%VERTCOLORS%USE_DAMAGE_MAP%WEAR_LAYER")
        .surface_type("metal_solid")
        .diffuse([0.5; 3])
        .layer_with(&[
            ("Name", "Primary"),
            ("Path", "Objects/Test/layers/base.mtl"),
            ("Submtl", ""),
            ("TintColor", "0.75,0.5,0.25"),
            ("WearTint", "1,1,1"),
            ("GlossMult", "0.5"),
            ("UVTiling", "2"),
            ("PaletteTint", "1"),
        ])
        .layer_with(&[
            ("Name", "Wear"),
            ("Path", "Objects/Test/layers/wear.mtl"),
            ("TintColor", "x"),
            ("PaletteTint", "0"),
        ])
        .layer_with(&[("Name", "Lost"), ("Path", "Objects/Test/layers/lost.mtl")])
        .layer_with(&[("Name", "Loop"), ("Path", "Objects/Test/layers/loop.mtl")])
        .public_param("WearBlendBase", "0.25")
        .public_param("DamageTiling", "5");
    let decal = TestMtlMaterial::new("decal")
        .shader("MeshDecal")
        .string_gen_mask("%VERTDATA%STENCIL_MAP%USE_DAMAGE_MAP")
        .texture("TexSlot7", "$TintPaletteDecal")
        .texture_with_mod(
            "TexSlot1",
            &texture("decal_diff"),
            &[("TileU", "3"), ("OffsetV", "0.5")],
        )
        .public_param("StencilDiffuseColor", "0.1,0.2,0.3")
        .public_param("StencilOpacity", "1");
    let canopy = TestMtlMaterial::new("canopy")
        .shader("GlassPBR")
        .string_gen_mask("%NORMAL_MAP%USE_TINT_PALETTE")
        .texture("TexSlot2", &texture("canopy_ddna"))
        .public_param("GlareStrength", "0.05");
    let mystery = TestMtlMaterial::new("mystery")
        .shader("ZedPBR")
        .opacity(0.5)
        .alpha_test(0.4)
        .attribute("ZedFlag", "7")
        .public_param("ZedStrength", "1.5")
        .public_param("ZedMode", "odd")
        .child(
            TestElement::new("ZedExtension")
                .attribute("Level", "3")
                .child(TestElement::new("Inner").attribute("A", "b")),
        );
    TestMtl::multi(&[TestMtlMaterial::new("unused"), hull, decal, canopy, mystery])
        .with_default_palette("Libs/Foundry/Records/TintPalettes/brand/test/p_layered")
        .cryxml()
}

fn layer_file(textures: &[(&str, &str)], params: &[(&str, &str)], layer: Option<&str>) -> Vec<u8> {
    let material = textures.iter().fold(
        TestMtlMaterial::new("layer").shader("Layer"),
        |material, (slot, name)| material.texture(slot, &texture(name)),
    );
    let material = params.iter().fold(material, |material, (name, value)| {
        material.public_param(name, value)
    });
    let material = match layer {
        Some(path) => material.layer_with(&[("Name", "Self"), ("Path", path)]),
        None => material,
    };
    TestMtl::single(&material).cryxml()
}

fn model() -> TestModel {
    let part = |material_id, first_index, index_count, first_vertex, vertex_count| TestSubmesh {
        material_id,
        node: 1,
        first_index,
        index_count,
        first_vertex,
        vertex_count,
    };
    TestModel::cube()
        .with_colours(vec![CUBE_COLOUR; CUBE_VERTICES])
        .with_submeshes(vec![
            part(CUBE_CAPS_MATERIAL, 0, 12, 0, 8),
            part(CUBE_SIDES_MATERIAL, 12, 12, 8, 8),
            part(3, 24, 6, 16, 4),
            part(4, 30, 6, 20, 4),
        ])
}

/// Returns the Entries of a synthetic Archive with a ship Entity `TEST_Layered` whose library
/// `Objects/Test/layered` has layered, decal, glass and unknown-shader materials.
///
/// The cube draws the material IDs 1 to 4:
///
/// - `hull`, a `HardSurface` material with four layers: `Primary` takes the primary palette
///   colour and names `layers/base.mtl`, `Wear` names `layers/wear.mtl` and has a malformed
///   `TintColor`, `Lost` names a file the Archive lacks and `Loop` names `layers/loop.mtl`, whose
///   only layer names itself;
/// - `decal`, a `MeshDecal` material with the placeholder `$TintPaletteDecal` and a decal texture
///   with a `TexMod`;
/// - `canopy`, a `GlassPBR` material that uses the tint palette;
/// - `mystery`, a material of the shader `ZedPBR` with unknown attributes, parameters and a child
///   element.
///
/// The ship chooses the palette `p_layered` ([`LAYERED_TINTS`]). The textures are 8 by 8 pixels,
/// except [`LAYERED_MISSING_TEXTURE`], which the Archive does not hold.
#[must_use]
pub fn layered_entries() -> Vec<TestEntry> {
    let model = model();
    let mut entries = vec![
        TestEntry::zstd("Data\\Game2.dcb", database().build()),
        TestEntry::stored("Data\\Objects\\Test\\layered_ship.cgf", model.cgf()),
        TestEntry::stored("Data\\Objects\\Test\\layered_ship.cgfm", model.cgfm()),
        TestEntry::stored("Data\\Objects\\Test\\layered.mtl", library()),
        TestEntry::stored(
            "Data\\Objects\\Test\\layers\\base.mtl",
            layer_file(
                &[
                    ("TexSlot1", "base_diff"),
                    ("TexSlot2", "base_ddna"),
                    ("TexSlot7", "base_detail"),
                    ("TexSlot8", "base_blend"),
                ],
                &[("LayerTiling", "1"), ("TintMode", "0")],
                None,
            ),
        ),
        TestEntry::stored(
            "Data\\Objects\\Test\\layers\\wear.mtl",
            layer_file(
                &[
                    ("TexSlot1", "wear_diff"),
                    ("TexSlot2", "wear_ddna"),
                    ("TexSlot6", "wear_spec"),
                ],
                &[("TintMode", "1")],
                None,
            ),
        ),
        TestEntry::stored(
            "Data\\Objects\\Test\\layers\\loop.mtl",
            layer_file(&[], &[], Some("Objects/Test/layers/loop.mtl")),
        ),
    ];
    let dds = SplitDds::new(TestTextureFormat::LegacyDxt1, SIDE, SIDE)
        .mips(4)
        .split(1);
    for name in LAYER_TEXTURES {
        entries.extend(dds.entries(&format!(
            "Data\\Objects\\Test\\layers\\textures\\{name}.dds"
        )));
    }
    entries
}
