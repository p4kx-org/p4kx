// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Reading the headers of `.caf` files and `.dba` databases without decoding a key.

use std::error::Error;

use p4kx_chunks::ChunkFile;
use p4kx_model::{
    Animation, AnimationError, AnimationHeader, Bone, CompanionError, CompanionPath,
    CorruptAnimation, Dba, DbaAnimationSources, DbaHeaderDeclarations, DbaHeaders,
    MAX_ANIMATION_KEYS, MAX_DATABASE_KEYS, Model,
};
use p4kx_testkit::{TestAnimation, TestDba, TestModel, TestSkeleton, TestTrack};

type TestResult = Result<(), Box<dyn Error>>;

const IDENTITY: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

fn bones(count: u16) -> Result<Vec<Bone>, Box<dyn Error>> {
    let rig = TestModel::cube()
        .helpers_only()
        .with_skeleton(&TestSkeleton::chain(count));
    let file = ChunkFile::parse(rig.cgf())?;
    let mut companions =
        |_: CompanionPath<'_>| -> Result<Option<Vec<u8>>, CompanionError> { Ok(None) };
    Ok(Model::read("rig.chr", &file, &mut companions)?.bones)
}

fn sample() -> TestAnimation {
    TestAnimation::new("animations/walk/forward.caf", 30)
        .additive()
        .track(
            TestTrack::new(TestSkeleton::controller_id(1))
                .rotations(&[(0, IDENTITY), (10, IDENTITY), (45, IDENTITY)])
                .positions(&[(0, [0.0; 3]), (45, [1.0, 2.0, 3.0])]),
        )
        .track(TestTrack::new(TestSkeleton::controller_id(7)).rotations(&[(0, IDENTITY)]))
        .track(TestTrack::new(TestSkeleton::controller_id(1)).rotations(&[(0, IDENTITY)]))
        .track(TestTrack::new(9))
}

#[test]
fn a_caf_header_says_what_the_decoded_animation_says() -> TestResult {
    let file = ChunkFile::parse(sample().caf())?;
    let header = AnimationHeader::read_caf("forward.caf", &file)?;
    let full = Animation::read_caf("forward.caf", &file)?;

    assert_eq!(header.name, full.name, "name");
    assert_eq!(header.path, full.path, "path");
    assert_eq!(header.frames_per_second, full.frames_per_second, "rate");
    assert_eq!(header.end_frame, full.end_frame, "end frame");
    assert_eq!(header.additive, full.additive, "additive");
    assert_eq!(header.keys, full.key_count(), "keys");
    let ids: Vec<u32> = full
        .tracks
        .iter()
        .map(|track| track.controller_id)
        .collect();
    assert_eq!(
        header.controller_ids, ids,
        "tracks without keys are dropped"
    );
    assert!(
        (header.duration_seconds() - full.duration_seconds()).abs() < f32::EPSILON,
        "duration"
    );
    Ok(())
}

#[test]
fn the_tracks_a_header_binds_are_the_tracks_the_animation_binds() -> TestResult {
    let bones = bones(4)?;
    let file = ChunkFile::parse(sample().caf())?;
    let header = AnimationHeader::read_caf("forward.caf", &file)?;
    let full = Animation::read_caf("forward.caf", &file)?;

    assert_eq!(
        header.bound_tracks(&bones),
        full.bind(&bones).bound.len(),
        "a second track for a bone and a bone the skeleton lacks are unbound"
    );
    assert_eq!(header.bound_tracks(&[]), 0, "no bones");
    Ok(())
}

#[test]
fn a_database_header_lists_the_animations_in_order() -> TestResult {
    let dba = TestDba::new().animation(sample()).animation(
        TestAnimation::new("animations/walk/back.caf", 60)
            .track(TestTrack::new(3).rotations(&[(0, IDENTITY), (60, IDENTITY)])),
    );
    let file = ChunkFile::parse(dba.bytes())?;

    let headers = DbaHeaders::read("pack.dba", &file)?;
    let full = Dba::read("pack.dba", &file)?;

    assert_eq!(headers.animations.len(), full.animations.len(), "count");
    assert!(headers.skipped.is_empty(), "nothing skipped");
    for (header, full) in headers.animations.iter().zip(&full.animations) {
        assert_eq!(header.name, full.name, "name");
        assert_eq!(header.path, full.path, "path");
        assert_eq!(header.end_frame, full.end_frame, "end frame");
        assert_eq!(header.keys, full.key_count(), "keys of {}", full.name);
        assert_eq!(header.controller_ids.len(), full.tracks.len(), "tracks");
    }
    Ok(())
}

#[test]
fn a_header_does_not_notice_a_key_format_the_reader_does_not_know() -> TestResult {
    let dba = TestDba::new().animation(
        TestAnimation::new("bad.caf", 30).track(
            TestTrack::new(1)
                .rotation_code(0x99)
                .rotations(&[(0, IDENTITY)]),
        ),
    );
    let file = ChunkFile::parse(dba.bytes())?;

    let headers = DbaHeaders::read("pack.dba", &file)?;
    let full = Dba::read("pack.dba", &file)?;

    assert_eq!(headers.animations.len(), 1, "listed from its header");
    assert_eq!(full.skipped.len(), 1, "skipped when decoded");
    Ok(())
}

#[test]
fn a_header_refuses_what_the_decoder_refuses_before_the_keys() -> TestResult {
    let file = ChunkFile::parse(TestAnimation::new("zero.caf", 0).caf())?;

    assert!(
        AnimationHeader::read_caf("zero.caf", &file).is_err(),
        "a frame rate of zero"
    );
    Ok(())
}

struct OversizedDba {
    bytes: Vec<u8>,
    suffix_at: usize,
    dense_clip_count: u64,
    dense_key_count: u64,
}

fn oversized_dba() -> Result<OversizedDba, Box<dyn Error>> {
    let keys_per_controller = 2 * u64::from(u16::MAX);
    let controllers_per_clip = MAX_ANIMATION_KEYS / keys_per_controller;
    let keys_per_clip = controllers_per_clip * keys_per_controller;
    let dense_clip_count = MAX_DATABASE_KEYS / keys_per_clip + 1;
    let dense_clip = (0..u32::try_from(controllers_per_clip)?)
        .fold(TestAnimation::new("dense.caf", 30), |clip, id| {
            clip.track(TestTrack::new(id))
        });
    let suffix =
        TestAnimation::new("suffix.caf", 30).track(TestTrack::new(1).rotations(&[(0, IDENTITY)]));
    let mut bytes = (0..dense_clip_count)
        .fold(TestDba::new(), |dba, _| dba.animation(dense_clip.clone()))
        .animation(suffix)
        .bytes();
    let (count_words, suffix_at) = declaration_offsets(&bytes)?;
    for offset in count_words {
        bytes
            .get_mut(offset..offset + size_of::<u16>())
            .ok_or("channel key count")?
            .copy_from_slice(&u16::MAX.to_le_bytes());
    }
    Ok(OversizedDba {
        bytes,
        suffix_at,
        dense_clip_count,
        dense_key_count: dense_clip_count * keys_per_clip,
    })
}

fn declaration_offsets(bytes: &[u8]) -> Result<(Vec<usize>, usize), Box<dyn Error>> {
    let file = ChunkFile::parse(bytes)?;
    let clips = DbaAnimationSources::read("fixture.dba", &file)?.collect::<Result<Vec<_>, _>>()?;
    let (suffix, dense) = clips.split_last().ok_or("suffix clip")?;
    let mut count_words = Vec::new();
    for clip in dense {
        for controller in clip.controllers()? {
            let table = controller.table_range();
            let translation = table.start + (table.end - table.start) / 2;
            count_words.extend([usize::try_from(table.start)?, usize::try_from(translation)?]);
        }
    }
    Ok((
        count_words,
        usize::try_from(suffix.controller_block_range().start)?,
    ))
}

#[test]
fn dba_declarations_count_and_validate_blocks_beyond_the_preview_budget() -> TestResult {
    let mut fixture = oversized_dba()?;
    let file = ChunkFile::parse(fixture.bytes.as_slice())?;
    for refusal in [
        DbaHeaders::read("fixture.dba", &file).err(),
        Dba::read("fixture.dba", &file).err(),
    ] {
        assert!(
            matches!(
                refusal,
                Some(AnimationError::Corrupt {
                    source: CorruptAnimation::TooManyKeys { keys, limit: MAX_DATABASE_KEYS },
                    ..
                }) if keys == fixture.dense_key_count
            ),
            "the ordinary reader stops before the suffix"
        );
    }
    let complete = DbaHeaderDeclarations::read("fixture.dba", &file)?;
    assert_eq!(
        complete.clips,
        fixture.dense_clip_count + 1,
        "includes the suffix clip"
    );
    assert_eq!(
        complete.keys,
        fixture.dense_key_count + 1,
        "includes the suffix key"
    );
    assert_eq!(
        complete.preview_budget_refusals(),
        1,
        "aggregate preview refusal"
    );

    *fixture
        .bytes
        .get_mut(fixture.suffix_at)
        .ok_or("suffix block")? = 0;
    let broken = ChunkFile::parse(fixture.bytes.as_slice())?;
    assert!(
        matches!(
            DbaHeaderDeclarations::read("fixture.dba", &broken),
            Err(AnimationError::Corrupt {
                source: CorruptAnimation::Signature { .. },
                ..
            })
        ),
        "a bad block after the preview refusal still fails validation"
    );
    Ok(())
}
