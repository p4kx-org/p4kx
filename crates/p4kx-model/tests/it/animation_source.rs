// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_chunks::{ChunkFile, ivo};
use p4kx_model::{
    Animation, AnimationController, AnimationError, AnimationSource, CorruptAnimation,
    DbaAnimationSources, MAX_ANIMATION_KEYS,
};
use p4kx_testkit::{
    TestAnimation, TestChunkFile, TestDba, TestPositionFormat, TestTimes, TestTrack,
};

type TestResult = Result<(), Box<dyn Error>>;

const ANIM_INFO: u32 = 0x4733_C6ED;
const CAF_DATA: u32 = 0xA949_6CB5;
const IDENTITY: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

#[test]
fn caf_reports_original_words_and_all_controller_rows() -> TestResult {
    let animation = TestAnimation::new("walk.caf", 30)
        .track(TestTrack::new(7))
        .track(
            TestTrack::new(7)
                .times(TestTimes::Bitmap)
                .rotations(&[(2, IDENTITY), (15, IDENTITY), (39, IDENTITY)])
                .position_format(TestPositionFormat::Planar)
                .positions(&[(3, [0.0, 2.0, 3.0]), (29, [1.0, 5.0, 9.0])]),
        );
    let mut metadata = animation.anim_info();
    let flags = 0x8000_0023_u32;
    metadata
        .get_mut(..4)
        .ok_or("flags")?
        .copy_from_slice(&flags.to_le_bytes());
    metadata
        .get_mut(12..16)
        .ok_or("metadata word")?
        .copy_from_slice(&[6, 8, 1, 2]);
    let bytes = TestChunkFile::ivo()
        .chunk(ANIM_INFO, 0x901, metadata.clone())
        .chunk(CAF_DATA, 0x900, animation.caf_data())
        .bytes();
    let file = ChunkFile::parse(bytes.as_slice())?;

    let clip = AnimationSource::read_caf("walk.caf", &file)?;
    let metadata_at = file
        .chunks_of(ivo::ANIM_INFO)
        .next()
        .ok_or("AnimInfo")?
        .1
        .offset();
    let data_at = file
        .chunks_of(ivo::CAF_DATA)
        .next()
        .ok_or("CafData")?
        .1
        .offset();

    assert_eq!(clip.ordinal(), 0);
    assert!(clip.database_name().is_none());
    assert_eq!(clip.metadata().version(), 0x901);
    assert_eq!(clip.metadata().range(), metadata_at..metadata_at + 48);
    assert_eq!(clip.metadata().bytes().as_slice(), metadata.as_slice());
    assert_eq!(clip.metadata().flags(), flags);
    assert_eq!(clip.metadata().timing()?.end_frame, 39);
    assert!(clip.metadata().timing()?.additive);
    assert_eq!(clip.controller_block_range(), data_at..data_at + 68);
    let mut controllers = clip.controllers()?;
    let empty = controllers.next().ok_or("empty controller")?;
    let keyed = controllers.next().ok_or("keyed controller")?;
    assert!(controllers.next().is_none());
    assert_eq!(empty.ordinal(), 0);
    assert_eq!(keyed.ordinal(), 1);
    assert_eq!(empty.controller_id(), keyed.controller_id());
    assert!(empty.decode()?.is_none());
    assert_eq!(keyed.id_range(), data_at + 16..data_at + 20);
    assert_eq!(keyed.table_range(), data_at + 44..data_at + 68);
    assert_eq!(keyed.rotation().packed_format, 0x8242);
    assert_eq!(keyed.translation().packed_format, 0xc242);
    assert_bitmap_keys(&keyed)
}

fn assert_bitmap_keys(keyed: &AnimationController<'_>) -> TestResult {
    let decoded = keyed.decode()?.ok_or("decoded controller")?;
    assert_eq!(
        decoded.rotations.ok_or("rotations")?.frames,
        [2, 15, 39],
        "rotation frames"
    );
    let positions = decoded.translations.ok_or("translations")?;
    assert_eq!(positions.frames, [3, 29], "translation frames");
    for (actual, expected) in positions
        .values
        .iter()
        .flatten()
        .zip([0.0, 2.0, 3.0, 1.0, 5.0, 9.0])
    {
        assert!(
            (actual - expected).abs() < 1e-3,
            "decoded axis {actual} differs from {expected}"
        );
    }
    Ok(())
}

#[test]
fn dba_keeps_ordinals_and_bad_metadata_without_losing_other_clips() -> TestResult {
    let skipped = TestAnimation::new("bad.caf", 0).additive();
    let good = TestAnimation::new("dir/walk.caf", 60).track(
        TestTrack::new(17)
            .rotation_code(0x85)
            .rotations(&[(0, IDENTITY)])
            .positions(&[(1, [1.0, 2.0, 3.0]), (7, [4.0, 5.0, 6.0])]),
    );
    let bytes = TestDba::new()
        .animation(skipped.clone())
        .animation(good.clone())
        .pool_after_metadata()
        .bytes();
    let file = ChunkFile::parse(bytes.as_slice())?;
    let metadata_at = file
        .chunks_of(ivo::DBA_METADATA)
        .next()
        .ok_or("DBA metadata")?
        .1
        .offset();

    let mut clips = DbaAnimationSources::read("pack.dba", &file)?;
    let first = clips.next().ok_or("first clip")??;
    let second = clips.next().ok_or("second clip")??;
    assert!(clips.next().is_none());
    assert_eq!(first.ordinal(), 0);
    assert_eq!(second.ordinal(), 1);
    assert_eq!(first.metadata().version(), 0x902);
    assert_eq!(
        first.metadata().bytes().as_slice(),
        skipped.anim_info().as_slice()
    );
    assert_eq!(
        second.metadata().bytes().as_slice(),
        good.anim_info().as_slice()
    );
    assert_eq!(first.metadata().range(), metadata_at + 8..metadata_at + 56);
    assert_eq!(
        second.metadata().range(),
        metadata_at + 56..metadata_at + 104
    );
    assert_eq!(first.metadata().timing(), Err(CorruptAnimation::FrameRate));
    let name = second.database_name().ok_or("database name")?;
    assert_eq!(name.bytes(), b"dir/walk.caf");
    assert_eq!(name.range(), metadata_at + 112..metadata_at + 125);
    let controller = second.controllers()?.next().ok_or("controller")?;
    assert_independent_channels(&controller)
}

fn assert_independent_channels(controller: &AnimationController<'_>) -> TestResult {
    assert_eq!(
        controller.rotation().packed_format,
        0x8540,
        "original rotation word"
    );
    let translations = controller
        .decode_translation()?
        .ok_or("usable translation")?;
    assert_eq!(
        translations.frames,
        [1, 7],
        "independent translation frames"
    );
    assert!(
        matches!(
            controller.decode(),
            Err(CorruptAnimation::UnknownFormat {
                kind: "rotation",
                code: 0x85
            })
        ),
        "the combined decoder reports the unsupported rotation"
    );
    Ok(())
}

#[test]
fn short_caf_metadata_cannot_borrow_bytes_from_the_next_chunk() -> TestResult {
    let animation = TestAnimation::new("short.caf", 30);
    let bytes = TestChunkFile::ivo()
        .chunk(
            ANIM_INFO,
            0x901,
            animation
                .anim_info()
                .into_iter()
                .take(20)
                .collect::<Vec<_>>(),
        )
        .chunk(CAF_DATA, 0x900, animation.caf_data())
        .bytes();
    let file = ChunkFile::parse(bytes)?;

    assert!(matches!(
        AnimationSource::read_caf("short.caf", &file),
        Err(AnimationError::Corrupt {
            chunk: "AnimInfo",
            source: CorruptAnimation::Truncated(_),
            ..
        })
    ));
    Ok(())
}

#[test]
fn dba_reads_shared_keys_beyond_the_data_chunk_and_preserves_empty_names() -> TestResult {
    let animation = TestAnimation::new("", 24).track(
        TestTrack::new(21)
            .times(TestTimes::Bitmap)
            .positions(&[(1, [0.0, 2.0, 3.0]), (37, [0.0, 5.0, 9.0])]),
    );
    let file = ChunkFile::parse(
        TestDba::new()
            .animation(animation)
            .pool_after_metadata()
            .bytes(),
    )?;

    let clip = DbaAnimationSources::read("shared.dba", &file)?
        .next()
        .ok_or("shared clip")??;
    let name = clip.database_name().ok_or("stored empty name")?;
    assert_eq!(name.bytes(), b"");
    assert_eq!(name.range().end - name.range().start, 1);
    let keys = clip
        .controllers()?
        .next()
        .ok_or("shared controller")?
        .decode()?
        .ok_or("decoded shared keys")?;
    assert_eq!(keys.translations.ok_or("translation")?.frames, [1, 37]);
    Ok(())
}

#[test]
fn the_source_inventory_can_visit_counts_above_the_eager_decoder_limit() -> TestResult {
    let track = TestTrack::new(1)
        .rotations(&[(0, IDENTITY)])
        .positions(&[(0, [0.0; 3])]);
    let tracks = 34;
    let animation = (0..tracks).fold(TestAnimation::new("large.caf", 30), |clip, _| {
        clip.track(track.clone())
    });
    let mut data = animation.caf_data();
    let table_at = 12 + 4 * tracks;
    let table_end = table_at + 24 * tracks;
    for channel in data
        .get_mut(table_at..table_end)
        .ok_or("controller table")?
        .as_chunks_mut::<12>()
        .0
    {
        channel
            .get_mut(..2)
            .ok_or("channel count")?
            .copy_from_slice(&u16::MAX.to_le_bytes());
    }
    let file = ChunkFile::parse(
        TestChunkFile::ivo()
            .chunk(ANIM_INFO, 0x901, animation.anim_info())
            .chunk(CAF_DATA, 0x900, data)
            .bytes(),
    )?;

    let clip = AnimationSource::read_caf("large.caf", &file)?;
    let keys: u64 = clip
        .controllers()?
        .map(|controller| {
            u64::from(controller.rotation().key_count)
                + u64::from(controller.translation().key_count)
        })
        .sum();
    assert!(keys > MAX_ANIMATION_KEYS);
    assert!(matches!(
        Animation::read_caf("large.caf", &file),
        Err(AnimationError::Corrupt {
            source: CorruptAnimation::TooManyKeys { .. },
            ..
        })
    ));
    Ok(())
}
