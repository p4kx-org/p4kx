// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::chunks::ivo;
use p4kx_extraction::{Archive, OpenOptions};
use p4kx_testkit::{
    ArchiveBuilder, SplitDds, TestAnimation, TestChunkFile, TestContainer, TestEntry, TestModel,
    TestTextureFormat, TestTrack, TestVorbis, test_key,
};

use super::TestResult;

pub(super) const DUPLICATE: &str = "Data/mixed/opaque.bin";
pub(super) const FIRST: &[u8] = b"first physical opaque payload";
pub(super) const SECOND: &[u8] = b"second physical opaque payload";
pub(super) const CACHE: &str = "Shaders/mixed.shcb";
pub(super) const CACHE_BYTES: &[u8] = b"opaque compiled shader";
pub(super) const FILTERED: &str = "Data/mixed/filtered.bnk";

const CLIP: &str = "Data/mixed/clip.caf";
const PARAMS: &[u8] = br#"<Params><AnimationList><Animation name="test" path="Data/mixed/clip.caf"/></AnimationList></Params>"#;
const MATERIAL: &[u8] = br#"<Material Name="Mixed" Shader="Illum"><Textures><Texture Map="Diffuse" File="Data/mixed/surface.dds"/></Textures><Unknown Kept="source"/></Material>"#;
const ATL: &[u8] = br#"<ATLConfig><AudioTriggers><ATLTrigger atl_name="Mixed"><WwiseEvent wwise_name="MixedEvent" wwise_localised="true"/></ATLTrigger></AudioTriggers></ATLConfig>"#;

pub(super) fn create(path: &Path) -> TestResult {
    let nested = TestContainer::new()
        .entry(TestEntry::stored(DUPLICATE, FIRST))
        .entry(TestEntry::stored(DUPLICATE, SECOND))
        .entry(TestEntry::stored(
            "Shaders/Illum.ext",
            b"Property { Name=%Mixed; Mask(0x1); }",
        ));
    let texture = SplitDds::new(TestTextureFormat::LegacyBgra8, 4, 4).split(1);
    let model = TestChunkFile::ivo()
        .chunk(ivo::SKIN_2.id(), 0x900, TestModel::cube().skin_chunk())
        .chunk(0x0BAD_BEEF, 0x900, b"opaque model chunk")
        .bytes();
    let mut archive = ArchiveBuilder::new()
        .entry(TestEntry::stored("mixed.pak", nested.build()).encrypted())
        .entry(TestEntry::stored(CACHE, CACHE_BYTES))
        .entry(TestEntry::stored("Data/mixed/shape.skinm", model))
        .entry(TestEntry::stored("Data/mixed/shape.mtl", MATERIAL))
        .entry(TestEntry::stored(CLIP, clip()))
        .entry(TestEntry::stored("Data/mixed/shape.chrparams", PARAMS))
        .entry(TestEntry::stored(
            "Data/mixed/voice.wem",
            TestVorbis::new(1, 24_000, &[false; 5]).wem().bytes(),
        ))
        .entry(TestEntry::stored("Data/Libs/GameAudio/Mixed.xml", ATL))
        .entry(TestEntry::stored(FILTERED, b"intentionally unselected"));
    for entry in texture.entries("Data/mixed/surface.dds") {
        archive = archive.entry(entry);
    }
    archive.write_to(path)?;
    Ok(())
}

pub(super) fn open(path: &Path) -> TestResult<Archive> {
    Ok(Archive::open_with(
        path,
        OpenOptions::new().key(Some(test_key())),
    )?)
}

fn clip() -> Vec<u8> {
    let animation = TestAnimation::new("clip.caf", 30)
        .track(TestTrack::new(17).positions(&[(0, [0.0, 2.0, 3.0]), (3, [1.0, 5.0, 9.0])]));
    TestChunkFile::ivo()
        .chunk(0x4733_C6ED, 0x901, animation.anim_info())
        .chunk(0xA949_6CB5, 0x900, animation.caf_data())
        .bytes()
}
