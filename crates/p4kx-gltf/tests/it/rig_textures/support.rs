// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Fixtures that build a textured piece on a shared skeleton.

use std::error::Error;

use p4kx_chunks::ChunkFile;
use p4kx_gltf::{
    GlbOptions, GlbSummary, MaterialMode, RigPiece, write_rigged_glb, write_rigged_glb_with,
};
use p4kx_model::{CompanionError, CompanionPath, MaterialLibrary, MaterialStatus, Model};
use p4kx_testkit::{
    CheckedGlb, TestBone, TestModel, TestMtl, TestMtlMaterial, TestSkeleton, check_glb,
};

use crate::textures::support::Stub;

fn bone(name: &str, parent: Option<u16>, z: f32) -> TestBone {
    TestBone {
        name: name.to_owned(),
        parent,
        rotation: [0.0, 0.0, 0.0, 1.0],
        translation: [0.0, 0.0, z],
    }
}

pub(super) fn body() -> Result<Model, Box<dyn Error>> {
    let skeleton = TestSkeleton::new(vec![
        bone("pelvis", None, 0.0),
        bone("TIP", Some(0), 1.0),
        bone("root", Some(1), 1.0),
    ]);
    let source = TestModel::cube().helpers_only().with_skeleton(&skeleton);
    let mut none = |_: CompanionPath<'_>| -> Result<Option<Vec<u8>>, CompanionError> { Ok(None) };
    Ok(Model::read(
        "body.chr",
        &ChunkFile::parse(source.cgf())?,
        &mut none,
    )?)
}

pub(super) fn piece() -> Result<Model, Box<dyn Error>> {
    let source = TestModel::two_bone_skin();
    let mut companions = |path: CompanionPath<'_>| -> Result<Option<Vec<u8>>, CompanionError> {
        Ok(match path {
            CompanionPath::Sibling("piece.skinm") => Some(source.cgfm()),
            _ => None,
        })
    };
    Ok(Model::read(
        "piece.skin",
        &ChunkFile::parse(source.cgf())?,
        &mut companions,
    )?)
}

pub(super) fn library(material: &TestMtlMaterial) -> Result<MaterialStatus, Box<dyn Error>> {
    library_at("objects/test/piece", material)
}

pub(super) fn library_at(
    path: &str,
    material: &TestMtlMaterial,
) -> Result<MaterialStatus, Box<dyn Error>> {
    let materials = [material.clone(), material.clone(), material.clone()];
    let bytes = TestMtl::multi(&materials).cryxml();
    Ok(MaterialStatus::Loaded(MaterialLibrary::parse(
        path, &bytes,
    )?))
}

pub(super) fn hair_cap(file: &str) -> TestMtlMaterial {
    TestMtlMaterial::new("hair_scalp_m")
        .shader("HairPBR")
        .string_gen_mask("%HAIR_CAP")
        .diffuse([0.0, 0.0, 0.0])
        .texture("TexSlot1", file)
}

pub(super) const MALE_LIBRARY: &str = "objects/test/hair_v2/sc/male/m_hair_31";
pub(super) const MALE_SCALP: &str = "Objects/Test/hair_v2/sc/male/textures/m_hair_31_scalp.tif";
pub(super) const FEMALE_SCALP: &str = "Objects/Test/hair_v2/sc/female/textures/f_hair_31_scalp.tif";

pub(super) fn write(
    status: &MaterialStatus,
    stub: Option<&Stub>,
) -> Result<(CheckedGlb, GlbSummary), Box<dyn Error>> {
    let (body, piece) = (body()?, piece()?);
    let pieces = [RigPiece::new("head", &piece, status)];
    let options = GlbOptions::default().with_materials(MaterialMode::Textures);
    let mut bytes = Vec::new();
    let summary = match stub {
        Some(stub) => write_rigged_glb_with(&body.bones, &pieces, &options, stub, &mut bytes)?,
        None => write_rigged_glb(&body.bones, &pieces, &options, &mut bytes)?,
    };
    Ok((check_glb(&bytes)?, summary))
}

pub(super) fn skin_material() -> TestMtlMaterial {
    TestMtlMaterial::new("shader_head")
        .shader("HumanSkin_V2")
        .diffuse([0.5, 1.0, 0.25])
        .texture("TexSlot1", "Objects/Test/head_diff.tif")
        .texture("TexSlot2", "Objects/Test/head_ddna.tif")
}
