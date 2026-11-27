// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::io;
use std::path::Path;

use p4kx_assembly::{ArchiveModels, Armour, AssemblyOptions, BodyVariant};
use p4kx_blender::{CharacterScene, PackageError, PackageInfo};
use p4kx_chunks::ChunkFile;
use p4kx_extraction::Archive;
use p4kx_gltf::{GlbOptions, GltfError};
use p4kx_model::Animation;
use p4kx_testkit::{TestAnimation, TestSkeleton, TestTrack};

use super::support::{TestResult, fixture, names_in, never};

const CLIP_COUNT: usize = 7160;
const GLB: &str = "meshes/m0001.glb";
const BUDGET: &str = include_str!("../../../../blender_addon/p4kx_importer/glb-json-budget.bytes");

#[test]
fn the_character_package_checks_the_padded_json_before_replacing_an_existing_generation()
-> TestResult {
    let fixture = fixture(p4kx_testkit::armour_entries())?;
    let archive = Archive::open(&fixture.path)?;
    let models = ArchiveModels::new(&archive);
    let armour = Armour::build(BodyVariant::Male, &[], &models, &AssemblyOptions::default())?;
    let budget: usize = BUDGET.trim_ascii().parse()?;
    let model = ModelFixture { armour, models };
    let scratch = tempfile::tempdir()?;
    let output = scratch.path().join("package");

    let mut clips = (0..CLIP_COUNT)
        .map(|index| animation(index, 0))
        .collect::<Result<Vec<_>, _>>()?;
    let unpadded_name_size = model.raw_json_length(&clips)?;
    let padding = budget
        .checked_sub(unpadded_name_size)
        .ok_or("the unadjusted synthetic clips must fit below the shared budget")?;

    clips[0] = animation(0, padding)?;
    assert_eq!(
        model.raw_json_length(&clips)?,
        budget,
        "exact boundary fixture"
    );
    model.write_package(&clips, &output)?;
    let previous = files(&output)?;
    assert_eq!(
        json_length(&previous[0])?,
        budget,
        "published exact boundary length"
    );
    let siblings = names_in(scratch.path())?;

    clips[0] = animation(0, padding + 4)?;
    assert_eq!(
        model.raw_json_length(&clips)?,
        budget + 4,
        "raw GLB exceeds the package budget"
    );
    let result = model.write_package(&clips, &output);
    assert!(
        matches!(
            result,
            Err(PackageError::Mesh {
                id,
                source: GltfError::JsonBudgetExceeded { actual, budget: limit },
                ..
            }) if id == "m0001" && actual == (budget + 4) as u64 && limit == budget as u64
        ),
        "replacement fails with the actual padded size and effective budget"
    );
    assert_eq!(
        files(&output)?,
        previous,
        "the earlier generation stays intact"
    );
    assert_eq!(
        names_in(scratch.path())?,
        siblings,
        "owned staging is removed"
    );
    Ok(())
}

struct ModelFixture<'a> {
    armour: Armour,
    models: ArchiveModels<'a>,
}

impl ModelFixture<'_> {
    fn raw_json_length(&self, clips: &[Animation]) -> Result<usize, Box<dyn Error>> {
        let fitted = self.armour.animate(clips, &self.models);
        assert!(
            fitted.issues().is_empty(),
            "all synthetic animation channels fit the rig"
        );
        let mut bytes = Vec::new();
        self.armour
            .write_glb_animated(&fitted, &GlbOptions::default(), &mut bytes)?;
        let size = json_length(&bytes)?;
        assert_eq!(
            bytes.get(20 + size + 4..20 + size + 8),
            Some(b"BIN\0".as_slice()),
            "rigged GLB has real binary data"
        );
        Ok(size)
    }

    fn write_package(&self, clips: &[Animation], output: &Path) -> Result<(), PackageError> {
        let fitted = self.armour.animate(clips, &self.models);
        let scene = CharacterScene::new("synthetic", &self.armour, &fitted);
        p4kx_blender::write_character_package(
            &scene,
            &self.models.textures(),
            &GlbOptions::default(),
            &PackageInfo::default(),
            output,
            &never,
        )?;
        Ok(())
    }
}

fn animation(index: usize, padding: usize) -> Result<Animation, Box<dyn Error>> {
    let name = format!("synthetic_{index:06}{}.caf", "x".repeat(padding));
    let mut clip = TestAnimation::new(&name, 30);
    for bone in 0..3 {
        clip = clip.track(
            TestTrack::new(TestSkeleton::controller_id(bone))
                .rotations(&[(0, [0.0, 0.0, 0.0, 1.0]), (1, [0.0, 0.0, 0.0, 1.0])])
                .positions(&[(0, [0.0; 3]), (1, [0.0, 0.0, 1.0])]),
        );
    }
    Ok(Animation::read_caf(&name, &ChunkFile::parse(clip.caf())?)?)
}

fn json_length(bytes: &[u8]) -> Result<usize, Box<dyn Error>> {
    assert_eq!(bytes.get(0..4), Some(b"glTF".as_slice()), "valid GLB magic");
    assert_eq!(
        bytes.get(16..20),
        Some(b"JSON".as_slice()),
        "JSON is the first chunk"
    );
    let size = bytes.get(12..16).ok_or("JSON length in the GLB header")?;
    Ok(usize::try_from(u32::from_le_bytes(size.try_into()?))?)
}

fn files(output: &Path) -> io::Result<[Vec<u8>; 3]> {
    Ok([
        fs::read(output.join(GLB))?,
        fs::read(output.join("scene.json"))?,
        fs::read(output.join("receipt.json"))?,
    ])
}
