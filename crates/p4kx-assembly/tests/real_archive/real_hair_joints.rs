// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The beard, brows and lashes of the shipped characters are bound to the facial joints, and a
//! facial animation that opens the jaw moves them through the skin weights (ADR-0299).

use std::error::Error;
use std::io::Read;
use std::path::Path;

use p4kx_assembly::character::Chf;
use p4kx_assembly::{
    ArchiveModels, Armour, ArmourPart, AssemblyOptions, ForgeItems, Head, LoadoutResolver,
    ModelProvider,
};
use p4kx_extraction::{AnimationChoice, Archive, DataForgeEntry};
use p4kx_gltf::{GlbOptions, SampledAnimation};
use p4kx_model::{
    BoneMatch, BoneTransform, Influences, MAX_INFLUENCES, chained_world_poses, multiply_quaternions,
};
use p4kx_testkit::{check_glb, real_archive_key, real_archive_path};

type TestResult = Result<(), Box<dyn Error>>;

const SHIPPED: &str = "libs\\charactercustomizer\\";
const CLIP: &str = "facerom_t0_face";
const BEARD: &str = "beard_itemport";
const FOLLOWING: [&str; 3] = [BEARD, "eyebrow_itemport", "eyelashes_itemport"];
/// The least share of the weights of a following part that sits on facial joints; the measured
/// shares are 0.66 and more.
const MIN_FACIAL_SHARE: f32 = 0.5;
/// Every this many samples of the clip are evaluated.
const STRIDE: usize = 8;

fn conjugate([x, y, z, w]: [f32; 4]) -> [f32; 4] {
    [-x, -y, -z, w]
}

fn rotate(turn: [f32; 4], [x, y, z]: [f32; 3]) -> [f32; 3] {
    let [rx, ry, rz, _] =
        multiply_quaternions(multiply_quaternions(turn, [x, y, z, 0.0]), conjugate(turn));
    [rx, ry, rz]
}

/// Returns `base + factor * by`.
const fn offset([bx, by_, bz]: [f32; 3], [x, y, z]: [f32; 3], factor: f32) -> [f32; 3] {
    [
        factor.mul_add(x, bx),
        factor.mul_add(y, by_),
        factor.mul_add(z, bz),
    ]
}

fn apply(pose: &BoneTransform, at: [f32; 3]) -> [f32; 3] {
    offset(rotate(pose.rotation, at), pose.translation, 1.0)
}

fn apply_inverse(pose: &BoneTransform, at: [f32; 3]) -> [f32; 3] {
    rotate(conjugate(pose.rotation), offset(at, pose.translation, -1.0))
}

/// The poses of the skeleton at rest and in one sample of a clip, in the model's space.
struct Poses {
    rest: Vec<BoneTransform>,
    posed: Vec<BoneTransform>,
}

fn posed_at(armour: &Armour, sampled: &SampledAnimation, at: usize) -> Poses {
    let rest = &armour.skeleton().bones;
    let mut bones = rest.clone();
    for moved in &sampled.bones {
        if let (Some(bone), Some(&translation), Some(&rotation)) = (
            bones.get_mut(moved.bone),
            moved.translations.get(at),
            moved.rotations.get(at),
        ) {
            bone.relative = BoneTransform {
                rotation,
                translation,
            };
        }
    }
    Poses {
        rest: chained_world_poses(rest),
        posed: chained_world_poses(&bones),
    }
}

/// Returns how far the weights `weights` carry the vertex at `position` from where it is.
fn moved_by(position: [f32; 3], weights: &Influences, matched: &BoneMatch, poses: &Poses) -> f32 {
    let total = f32::from(u16::try_from(weights.sum()).unwrap_or(1)).max(1.0);
    let mut skinned = [0.0_f32; 3];
    for (&joint, &weight) in weights.joints.iter().zip(&weights.weights) {
        let joint = usize::from(matched.target(joint));
        let (Some(pose), Some(bind)) = (poses.posed.get(joint), poses.rest.get(joint)) else {
            continue;
        };
        let carried = apply(pose, apply_inverse(bind, position));
        skinned = offset(skinned, carried, f32::from(weight) / total);
    }
    offset(skinned, position, -1.0)
        .iter()
        .map(|value| value * value)
        .sum::<f32>()
        .sqrt()
}

fn part<'a>(armour: &'a Armour, name: &str) -> Option<&'a ArmourPart> {
    armour.parts().iter().find(|part| part.name() == name)
}

/// Returns by how much, at most, a vertex of `part` moves in the clip, every [`STRIDE`] samples.
fn largest_move(armour: &Armour, part: &ArmourPart, sampled: &SampledAnimation) -> Option<f32> {
    let lod = part.model().lods.first()?;
    let matched = BoneMatch::new(&armour.skeleton().bones, part.model().bones_of(lod));
    let mesh = lod.meshes.first()?;
    let influences = mesh.influences.as_ref()?;
    (0..sampled.times.len())
        .step_by(STRIDE)
        .map(|at| {
            let poses = posed_at(armour, sampled, at);
            mesh.positions
                .iter()
                .zip(influences)
                .map(|(&position, weights)| moved_by(position, weights, &matched, &poses))
                .fold(0.0_f32, f32::max)
        })
        .reduce(f32::max)
}

/// Returns the share of the vertices of `part` whose strongest bone is one of the facial joints,
/// the bones the body skeleton lacks, and the largest number of bones of a vertex.
fn facial_share(armour: &Armour, part: &ArmourPart, body_bones: usize) -> Option<(f32, usize)> {
    let lod = part.model().lods.first()?;
    let matched = BoneMatch::new(&armour.skeleton().bones, part.model().bones_of(lod));
    let influences = lod.meshes.first()?.influences.as_ref()?;
    let widest = influences.iter().map(Influences::count).max()?;
    let (mut facial, mut all) = (0_u64, 0_u64);
    for each in influences {
        for (&joint, &weight) in each.joints.iter().zip(&each.weights) {
            all += u64::from(weight);
            if usize::from(matched.target(joint)) >= body_bones {
                facial += u64::from(weight);
            }
        }
    }
    #[expect(clippy::cast_precision_loss, reason = "a share of millions of weights")]
    Some((facial as f32 / all.max(1) as f32, widest))
}

fn check_bound(armour: &Armour, path: &str, body_bones: usize) {
    for name in FOLLOWING {
        let Some((share, widest)) =
            part(armour, name).and_then(|found| facial_share(armour, found, body_bones))
        else {
            continue;
        };
        assert!(widest <= MAX_INFLUENCES, "{path}: {name}");
        assert!(
            share > MIN_FACIAL_SHARE,
            "{path}: {name} has {share} of its weight on facial joints"
        );
    }
}

#[expect(clippy::print_stdout, reason = "the distances are the measurement")]
fn check_animated(
    armour: &Armour,
    models: &ArchiveModels<'_>,
    path: &str,
) -> Result<(), Box<dyn Error>> {
    let set = models.animations(
        &armour.variant().animation_folders(),
        &AnimationChoice::Named(vec![CLIP.to_owned()]),
        &[],
    )?;
    let animations = armour.animate(&set.animations, models);
    let sampled = animations.sampled().first().ok_or("the sampled clip")?;
    for name in FOLLOWING {
        let Some(moved) = part(armour, name).and_then(|found| largest_move(armour, found, sampled))
        else {
            continue;
        };
        println!(
            "{path}: {name} moves by {:.1} mm at most in {CLIP}",
            moved * 1000.0
        );
        assert!(moved < 0.1, "{name} moves by {moved} m");
        let least = if name == BEARD { 0.005 } else { 0.001 };
        assert!(moved > least, "{name} moves by {moved} m");
    }
    let mut glb = Vec::new();
    armour.write_glb_animated(&animations, &GlbOptions::default(), &mut glb)?;
    check_glb(&glb)?;
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_beard_brows_and_lashes_of_the_shipped_characters_are_bound_to_the_facial_joints()
-> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, real_archive_key().ok())?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let models = ArchiveModels::new(&archive);
    let mut resolver = LoadoutResolver::new(&forge);
    let mut items = ForgeItems::new(&mut resolver);
    let (mut bearded, mut animated) = (0_usize, false);
    for file in archive.entries() {
        let path = file.path().to_ascii_lowercase();
        let is_chf = Path::new(&path)
            .extension()
            .is_some_and(|extension| extension == "chf");
        if !is_chf || !path.contains(SHIPPED) {
            continue;
        }
        let mut bytes = Vec::new();
        archive.open_entry(file)?.read_to_end(&mut bytes)?;
        let Ok(head) = Chf::read(&bytes)
            .map_err(Box::<dyn Error>::from)
            .and_then(|chf| Ok(Head::dressed(&chf.character, &mut items)?))
        else {
            continue;
        };
        let armour = Armour::build(
            head.variant(),
            head.pieces(),
            &models,
            &AssemblyOptions::default(),
        )?;
        let body_bones = models
            .model(armour.variant().skeleton(), 0)?
            .model
            .bones
            .len();
        check_bound(&armour, &path, body_bones);
        if part(&armour, BEARD).is_none() {
            continue;
        }
        bearded += 1;
        if !animated && armour.variant().code() == "male" {
            check_animated(&armour, &models, &path)?;
            animated = true;
        }
    }
    assert!(bearded >= 10, "{bearded} shipped beards");
    assert!(animated, "no bearded male character was animated");
    Ok(())
}
