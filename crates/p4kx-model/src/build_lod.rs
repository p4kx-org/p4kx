// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_chunks::ivo;

use crate::bone_map::check_joints;
use crate::build;
use crate::build::{Geometry, MAX_LOD, Rig, corrupt, fetch, first_of, read_meshes};
use crate::companion::lod_file_name;
use crate::model::MESH_TYPES;
use crate::skeleton_source;
use crate::{
    Bone, CompanionResolver, Lod, LodSelection, LodSkeletonOrigin, Mesh, MeshOrigin,
    ModelBoneSources, ModelChunk, ModelError, ModelFile, ModelIssue,
};

struct LevelSkeleton {
    bones: Option<Vec<Bone>>,
    origin: LodSkeletonOrigin,
}

/// Reads a level's skeleton and keeps the file's chunk usage before sharing effective bones.
fn level_skeleton(
    name: &str,
    level: u32,
    base: &[Bone],
    companions: &mut dyn CompanionResolver,
    chunks: &mut Vec<ModelChunk>,
    sources: Option<&mut Vec<ModelBoneSources>>,
    meshes: &[Mesh],
) -> Result<LevelSkeleton, ModelError> {
    if meshes.iter().all(|mesh| mesh.influences.is_none()) {
        return Ok(LevelSkeleton {
            bones: None,
            origin: LodSkeletonOrigin::NotRead,
        });
    }
    let file_name = lod_file_name(name, level);
    let which = ModelFile::LodModel(level);
    let Some(file) = fetch(companions, which, &file_name)? else {
        return Ok(LevelSkeleton {
            bones: None,
            origin: LodSkeletonOrigin::MissingFile,
        });
    };
    let mut read = Vec::new();
    let bones =
        skeleton_source::read_model_skeleton(&file_name, &file, which, &mut read, sources, None)?;
    chunks.extend(build::usage(&file, which, &read));
    let origin = if read.is_empty() {
        LodSkeletonOrigin::NoSkeleton
    } else if bones.is_empty() {
        LodSkeletonOrigin::EmptySkeleton
    } else if bones == base {
        LodSkeletonOrigin::EqualModel
    } else {
        LodSkeletonOrigin::Distinct
    };
    Ok(LevelSkeleton {
        bones: (origin == LodSkeletonOrigin::Distinct).then_some(bones),
        origin,
    })
}

/// Reads the geometry companions `<stem>_lod<n>.<extension>m` of the levels from 1 on that
/// `selection` wants; a level without companion is left out.
pub(crate) fn read_lod_companions(
    name: &str,
    companions: &mut dyn CompanionResolver,
    selection: LodSelection,
    rig: Rig<'_>,
    read: &mut Vec<(ModelFile, usize)>,
    found: &mut Geometry,
    mut sources: Option<&mut Vec<ModelBoneSources>>,
) -> Result<(), ModelError> {
    for level in (1..=MAX_LOD).filter(|&level| selection.wants(level)) {
        let companion_name = format!("{}m", lod_file_name(name, level));
        let Some(companion) = fetch(companions, ModelFile::LodGeometry(level), &companion_name)?
        else {
            continue;
        };
        let Some(index) = first_of(&companion, &MESH_TYPES) else {
            if companion.chunks_of(ivo::VIS_AREAS).next().is_some() {
                if selection.is_lenient() {
                    found.issues.push(ModelIssue::LodWithoutMesh {
                        level,
                        file: companion_name,
                    });
                }
                found.chunks.extend(build::usage(
                    &companion,
                    ModelFile::LodGeometry(level),
                    read,
                ));
                continue;
            }
            return Err(ModelError::NoMesh {
                file: companion_name,
            });
        };
        read.push((ModelFile::LodGeometry(level), index));
        let meshes = read_meshes(
            &companion_name,
            MeshOrigin::new(ModelFile::LodGeometry(level), index, level),
            companion.chunk_data(index),
            rig,
            false,
        )?;
        let skeleton = level_skeleton(
            name,
            level,
            rig.bones,
            companions,
            &mut found.chunks,
            sources.as_deref_mut(),
            &meshes,
        )?;
        let count = skeleton.bones.as_ref().map_or(rig.bones.len(), Vec::len);
        check_joints(&meshes, count).map_err(corrupt(&companion_name, index, "Skin"))?;
        found.lods.push(Lod {
            level,
            meshes,
            bones: skeleton.bones,
            skeleton_origin: skeleton.origin,
        });
        found.chunks.extend(build::usage(
            &companion,
            ModelFile::LodGeometry(level),
            read,
        ));
    }
    Ok(())
}
