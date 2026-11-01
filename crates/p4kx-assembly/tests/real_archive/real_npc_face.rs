// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The face of the story character Miles Eckhart of a real Archive: the joints of his own
//! `.RigLogic` file, the blendshapes of his own head geometry, and a face animation of his folder;
//! and the data those readers rest on, checked against the DNA rig of the stock head.

use std::error::Error;
use std::io::Read;

use p4kx_assembly::{
    ArchiveFiles, ArchiveModels, Armour, AssemblyOptions, Expressions, IssueKind, LoadoutResolver,
    ModelProvider,
};
use p4kx_character::{DeltaSet, DnaRig, FaceRig, RigLogic, SkinBlendshapes};
use p4kx_chunks::{ChunkFile, ivo};
use p4kx_extraction::{AnimationChoice, Archive, DataForgeEntry};
use p4kx_gltf::GlbOptions;
use p4kx_testkit::{check_glb, real_archive_key, real_archive_path};

use crate::real_paths::unified;

type TestResult = Result<(), Box<dyn Error>>;

const NPC: &str = "NPC_Archetypes-Male-Human-MissionGivers-MilesEckhart";
const ANIMATIONS: &str = "Animations/pu/male/eckhart";
const FACE_CLIP: &str = "pu_eckhart_mg_chosen_playeracceptmission_ig_002_face";
const RIG_LOGIC_FOLDER: &str = "objects/characters/riglogic/";
const WRINKLE_MASKS: &str = "shared_wrinkle_mask";
const GENERIC_HEAD: &str = "objects/characters/human/heads/male/pu/protos_human_male_face_t1_pu/protos_human_male_face_t1_pu_head";
/// The most a blendshape of the geometry may differ from the cell of the DNA rig: both hold byte
/// codes, and the largest step of the blendshapes compared is 2.5e-5 m.
const QUANTISATION: f32 = 5e-5;
/// The distance, 10 micrometres, from which two bind poses of a bone count as apart.
const APART: f32 = 1e-5;

fn read_entry(archive: &Archive, path: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let wanted = path.to_ascii_lowercase();
    let entry = archive
        .entries()
        .iter()
        .find(|entry| {
            entry
                .path()
                .to_ascii_lowercase()
                .replace('\\', "/")
                .ends_with(&wanted)
        })
        .ok_or_else(|| format!("the Archive has no {path}"))?;
    let mut bytes = Vec::new();
    archive.open_entry(entry)?.read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// Returns the farthest any joint of `face` moves from its first sample, on one axis.
fn reach(face: &p4kx_gltf::SampledAnimation) -> f32 {
    face.bones
        .iter()
        .filter_map(|bone| {
            let rest = *bone.translations.first()?;
            Some(
                bone.translations
                    .iter()
                    .flat_map(move |at| at.iter().zip(rest).map(|(a, b)| (a - b).abs())),
            )
        })
        .flatten()
        .fold(0.0_f32, f32::max)
}

/// Returns the head skin of Miles Eckhart and the skeleton file beside it.
fn head_models(
    models: &ArchiveModels<'_>,
) -> Result<(p4kx_model::Model, p4kx_model::Model), Box<dyn Error>> {
    let folder = "Objects/Characters/Human/heads/male/pu/eckhart";
    let load = |name: &str| {
        models
            .model(&format!("{folder}/{name}"), 0)
            .map(|provided| provided.model)
            .map_err(|failure| format!("{name}: {failure}"))
    };
    Ok((
        load("eckhart_t2_head.skin")?,
        load("eckhart_t2_head_skel.chr")?,
    ))
}

/// Builds Miles Eckhart at level of detail `lod` with `expressions` on his head.
fn eckhart(
    archive: &Archive,
    models: &ArchiveModels<'_>,
    expressions: &Expressions,
    lod: u32,
) -> Result<Armour, Box<dyn Error>> {
    let entry = DataForgeEntry::read(archive)?;
    let forge = entry.open()?;
    let files = ArchiveFiles::new(archive);
    let mut resolver = LoadoutResolver::new(&forge).with_files(&files);
    let record = resolver.entities().find(NPC)?;
    let tree = resolver.resolve(&record)?;
    let head = resolver
        .npc_head(&tree)
        .ok_or("no head")?
        .with_expressions(expressions);
    let mut pieces = resolver.worn(&tree);
    pieces.extend(head.pieces().iter().cloned());
    Ok(Armour::build(
        head.variant(),
        &pieces,
        models,
        &AssemblyOptions::default().with_lod(lod),
    )?)
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
#[expect(clippy::print_stdout, reason = "the counts are the measurement")]
fn miles_eckhart_has_his_expressions_at_the_lower_levels_too() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, real_archive_key().ok())?;
    let models = ArchiveModels::new(&archive);
    let count = |armour: &Armour| {
        armour
            .parts()
            .iter()
            .find(|part| part.name() == "head")
            .map_or(0, |part| part.morphs().len())
    };
    let base = count(&eckhart(&archive, &models, &Expressions::All, 0)?);
    for level in 1..=2 {
        let armour = eckhart(&archive, &models, &Expressions::All, level)?;
        println!("LOD {level}: {} of {base} expressions", count(&armour));
        assert!(count(&armour) > 300, "{}", count(&armour));
        assert!(count(&armour) <= base);
        assert!(
            !armour
                .issues()
                .iter()
                .any(|issue| issue.kind().code() == "morphs_failed"),
            "{:?}",
            armour.issues()
        );
    }
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
#[expect(clippy::print_stdout, reason = "the counts are the measurement")]
fn miles_eckhart_has_expressions_and_a_face_animation_of_his_own() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, real_archive_key().ok())?;
    let models = ArchiveModels::new(&archive);
    let armour = eckhart(&archive, &models, &Expressions::All, 0)?;

    let part = armour
        .parts()
        .iter()
        .find(|part| part.name() == "head")
        .ok_or("no head part")?;
    println!("{} expressions", part.morphs().len());
    assert!(part.morphs().len() > 300, "{}", part.morphs().len());
    let failed: Vec<_> = armour
        .issues()
        .iter()
        .filter(|issue| {
            matches!(
                issue.kind(),
                IssueKind::FaceJointsFailed { .. } | IssueKind::MorphsFailed { .. }
            )
        })
        .collect();
    assert!(failed.is_empty(), "{failed:?}");

    let found = models.animations(
        &[ANIMATIONS],
        &AnimationChoice::Named(vec![FACE_CLIP.to_owned()]),
        &[],
    )?;
    assert_eq!(found.animations.len(), 1);
    println!("{} tracks in the clip", found.animations[0].tracks.len());
    let animated = armour.animate(&found.animations, &models);
    let [face] = animated.sampled() else {
        return Err(format!("no face animation: {:?}", animated.issues()).into());
    };
    println!(
        "{} samples, {} joints move, {} weight runs, {} tracks left out",
        face.times.len(),
        face.bones.len(),
        face.weights.len(),
        face.skipped_tracks.len()
    );
    assert!(face.times.len() > 10);
    assert!(face.bones.len() > 20, "{} joints", face.bones.len());
    assert_ne!(face.weights.len(), 0);
    let reach = reach(face);
    println!("the farthest a joint moves: {reach} m");
    assert!(reach > 0.001, "{reach}");

    let mut bytes = Vec::new();
    armour.write_glb_animated(&animated, &GlbOptions::default(), &mut bytes)?;
    let glb = check_glb(&bytes)?;
    assert!(glb.document.animations().count() >= 1);
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
#[expect(clippy::print_stdout, reason = "the counts are the measurement")]
fn every_rig_logic_file_of_the_archive_reads_and_moves_its_joints() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, real_archive_key().ok())?;
    let (mut read, mut masks) = (0_usize, 0_usize);
    for entry in archive.entries() {
        let path = unified(entry.path());
        if !path.contains(RIG_LOGIC_FOLDER) || !path.ends_with(".riglogic") {
            continue;
        }
        if path.contains(WRINKLE_MASKS) {
            masks += 1;
            continue;
        }
        let mut bytes = Vec::new();
        archive.open_entry(entry)?.read_to_end(&mut bytes)?;
        let logic = RigLogic::read(&bytes).map_err(|error| format!("{path}: {error}"))?;
        FaceRig::from_logic(&logic).map_err(|error| format!("{path}: {error}"))?;
        read += 1;
    }
    println!("{read} rig logic files of heads, {masks} wrinkle mask files");
    assert!(read > 50, "{read}");
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_delta_records_of_a_head_skin_are_those_of_the_base_head_of_its_dna_rig() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, real_archive_key().ok())?;
    let skin = read_entry(&archive, &format!("{GENERIC_HEAD}.skin"))?;
    let chunks = ChunkFile::parse(skin.as_slice())?;
    let (index, _) = chunks
        .chunks_of(ivo::RIG_LOGIC)
        .next()
        .ok_or("no RigLogic")?;
    let logic = RigLogic::read(chunks.chunk_data(index).ok_or("chunk data")?)?;

    let dna_bytes = read_entry(&archive, &format!("{GENERIC_HEAD}.dna"))?;
    let rig = DnaRig::read(&dna_bytes)?;
    let deltas = rig.variant_deltas()?;
    let header = deltas.header(0).ok_or("no base head")?;
    for set in [DeltaSet::Main, DeltaSet::Secondary] {
        let own = logic.joint_records(set).ok_or("no records")?;
        let stock = deltas.records(0, set)?;
        assert_eq!(logic.joint_step(set), Some(header.step(set)));
        let mut count = 0_usize;
        for (own, stock) in own.zip(stock) {
            let (own, stock) = (own?, stock?);
            assert_eq!(own.output, stock.output);
            assert!(own.entries().eq(stock.entries()), "record {count}");
            count += 1;
        }
        assert_eq!(count, usize::try_from(header.records(set))?);
    }
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
#[expect(clippy::print_stdout, reason = "the counts are the measurement")]
fn the_blendshapes_of_a_head_geometry_are_the_cells_of_the_base_head_of_its_dna_rig() -> TestResult
{
    let archive = Archive::open_with_key(real_archive_path()?, real_archive_key().ok())?;
    let geometry = read_entry(&archive, &format!("{GENERIC_HEAD}.skinm"))?;
    let chunks = ChunkFile::parse(geometry.as_slice())?;
    let (index, _) = chunks
        .chunks_of(ivo::BSHAPES_GPU)
        .next()
        .ok_or("no BShapesGpu")?;
    let shapes = SkinBlendshapes::read(chunks.chunk_data(index).ok_or("chunk data")?)?;
    let dna_bytes = read_entry(&archive, &format!("{GENERIC_HEAD}.dna"))?;
    let rig = DnaRig::read(&dna_bytes)?;
    let morphs = rig.expression_morphs()?;
    assert_eq!(shapes.blendshapes(), morphs.blendshapes());

    let (mut compared, mut stock_moves, mut common_moves) = (0_usize, 0_usize, 0_usize);
    for blendshape in (1..shapes.blendshapes()).step_by(7) {
        let Some(cell) = morphs.cell(0, blendshape) else {
            continue;
        };
        let gpu = shapes.target(blendshape);
        for (vertex, delta) in cell.moves() {
            stock_moves += 1;
            let Ok(at) = gpu.vertices.binary_search(&vertex) else {
                continue;
            };
            common_moves += 1;
            for (axis, value) in delta.iter().enumerate() {
                let other = gpu.deltas[at][axis];
                assert!(
                    (value - other).abs() < QUANTISATION,
                    "{blendshape}: {value} against {other}"
                );
            }
        }
        compared += 1;
    }
    println!("{compared} blendshapes compared, {common_moves} of {stock_moves} moves in both");
    assert!(compared > 20, "{compared}");
    assert!(common_moves * 100 >= stock_moves * 90);
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
#[expect(clippy::print_stdout, reason = "the counts are the measurement")]
fn the_facial_joints_sit_where_the_skeleton_file_of_the_head_puts_them() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, real_archive_key().ok())?;
    let models = ArchiveModels::new(&archive);
    let armour = eckhart(&archive, &models, &Expressions::None, 0)?;
    let (skin, file) = head_models(&models)?;
    let (mut facial, mut like_the_file, mut unlike_the_skin) = (0_usize, 0_usize, 0_usize);
    for bone in &armour.skeleton().bones {
        let name = bone.name.to_ascii_lowercase();
        if !name.starts_with("facial_") {
            continue;
        }
        let find = |model: &p4kx_model::Model| {
            model
                .bones
                .iter()
                .find(|other| other.name.eq_ignore_ascii_case(&name))
                .map(|other| other.relative.translation)
        };
        let (Some(own), Some(stock)) = (find(&file), find(&skin)) else {
            continue;
        };
        let apart = |a: [f32; 3], b: [f32; 3]| a.iter().zip(b).any(|(a, b)| (a - b).abs() > APART);
        facial += 1;
        like_the_file += usize::from(!apart(bone.relative.translation, own));
        unlike_the_skin += usize::from(apart(bone.relative.translation, stock));
    }
    println!(
        "{facial} facial bones: {like_the_file} are the skeleton file's, {unlike_the_skin} differ from the skin's"
    );
    assert!(facial > 150, "{facial}");
    assert_eq!(like_the_file, facial);
    assert!(unlike_the_skin > 100, "{unlike_the_skin}");
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn every_bone_of_the_head_skin_is_in_its_skeleton_file_with_the_same_parent_and_id() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, real_archive_key().ok())?;
    let (skin, file) = head_models(&ArchiveModels::new(&archive))?;
    let parent_of = |model: &p4kx_model::Model, parent: Option<usize>| {
        parent
            .and_then(|index| model.bones.get(index))
            .map(|bone| bone.name.to_ascii_lowercase())
    };
    for bone in &skin.bones {
        let other = file
            .bones
            .iter()
            .find(|other| other.name.eq_ignore_ascii_case(&bone.name))
            .ok_or_else(|| format!("{} is not in the skeleton file", bone.name))?;
        assert_eq!(
            parent_of(&skin, bone.parent),
            parent_of(&file, other.parent),
            "{}",
            bone.name
        );
        if bone.parent.is_some() {
            assert_eq!(bone.controller_id, other.controller_id, "{}", bone.name);
        }
    }
    Ok(())
}
