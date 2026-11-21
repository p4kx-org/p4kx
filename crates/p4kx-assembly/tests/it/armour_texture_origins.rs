// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{ArchiveModels, Armour, AssemblyOptions, BodyVariant, Piece};
use p4kx_extraction::{
    Archive, GlbOptions, MaterialMode, SourceEncryptionState, SourceLocation, SourceUses,
};
use p4kx_gltf::TextureComponents;
use p4kx_testkit::{
    SplitDds, TestEntry, TestModel, TestMtl, TestMtlMaterial, TestTextureFormat, armour_entries,
    test_key,
};

use crate::assembly_support::{TestResult, fixture};

fn entries() -> Vec<TestEntry> {
    let cube = TestModel::cube().with_material_library("mapped");
    let material = TestMtl::single(
        &TestMtlMaterial::new("mapped").texture("TexSlot2", "Objects/Test/glass_ddna.tif"),
    );
    let dds = SplitDds::new(TestTextureFormat::LegacyBgra8, 8, 8)
        .mips(2)
        .split(1)
        .alpha(TestTextureFormat::R8);
    let mut entries = armour_entries();
    entries.extend([
        TestEntry::stored(r"Data\Objects\Test\mapped.cgf", cube.cgf()),
        TestEntry::stored(r"Data\Objects\Test\mapped.cgfm", cube.cgfm()),
        TestEntry::stored(r"Data\Objects\Test\mapped.mtl", material.cryxml()),
    ]);
    entries.extend(
        dds.entries(r"Data\Objects\Test\glass_ddna.dds")
            .into_iter()
            .map(|entry| {
                if entry.path().ends_with('a') {
                    entry.encrypted()
                } else {
                    entry
                }
            }),
    );
    entries
}

#[test]
fn rgb_and_gloss_images_bind_different_physical_inputs_from_one_decoded_dds() -> TestResult {
    let fixture = fixture(entries())?;
    let archive = Archive::open_with_key(&fixture.path, Some(test_key()))?;
    let models = ArchiveModels::new(&archive);
    let armour = Armour::build(
        BodyVariant::Male,
        &[Piece::file("mapped", "Objects/Test/mapped.cgf", None)],
        &models,
        &AssemblyOptions::default(),
    )?;
    let animated = armour.animate(&[], &models);
    let options = GlbOptions::default()
        .with_materials(MaterialMode::Textures)
        .with_texture_mip(1);
    let mut baseline = Vec::new();
    let expected =
        armour.write_glb_animated_with(&animated, &options, &models.textures(), &mut baseline)?;
    let mut actual = Vec::new();
    let textures = models.textures().observed();
    let (summary, emission) =
        armour.write_glb_animated_observed(&animated, &options, &textures, &mut actual)?;
    assert_eq!(actual, baseline);
    assert_eq!(summary, expected);
    assert_eq!(summary.textures, 2);
    let uses = SourceUses::new();
    for (inputs, (component, encryption)) in emission.image_sources().zip([
        (TextureComponents::Color, SourceEncryptionState::Unencrypted),
        (TextureComponents::Alpha, SourceEncryptionState::Encrypted),
    ]) {
        let [input] = inputs else {
            return Err("one source per embedded image is required for this fixture".into());
        };
        assert_eq!(input.components, component);
        let evidence = textures
            .source_uses(*input, &uses)?
            .ok_or("physical texture evidence")?;
        let mip = evidence.iter().find(|source| {
            matches!(source.description().location(), SourceLocation::Subresource { namespace, .. } if namespace == "ddsMipPayload")
        }).ok_or("mip payload evidence")?;
        assert_eq!(mip.encryption(), encryption);
    }
    Ok(())
}
