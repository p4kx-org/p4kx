// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_chunks::ChunkFile;
use p4kx_model::{AnimationSource, CorruptAnimation};
use p4kx_testkit::{TestAnimation, TestRotationFormat, TestTrack};

type TestResult = Result<(), Box<dyn Error>>;
const IDENTITY: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

fn classification(track: TestTrack) -> Result<Option<bool>, Box<dyn Error>> {
    let raw = TestAnimation::new("synthetic.caf", 30).track(track).caf();
    let file = ChunkFile::parse(raw.as_slice())?;
    let source = AnimationSource::read_caf("synthetic.caf", &file)?;
    let controller = source
        .controllers()?
        .next()
        .ok_or("one original controller")?;
    Ok(controller
        .rotation_identity()?
        .map(|evidence| evidence.is_canonical()))
}

#[test]
fn canonical_identity_is_defined_by_each_supported_encoding() -> TestResult {
    for format in [
        TestRotationFormat::Float,
        TestRotationFormat::SmallTree,
        TestRotationFormat::SmallTree64,
    ] {
        let identity = TestTrack::new(7)
            .rotation_format(format)
            .rotations(&[(0, IDENTITY), (9, IDENTITY)]);
        assert_eq!(
            classification(identity)?,
            Some(true),
            "the exact stock identity encoding: {format:?}"
        );
        let (sine, cosine) = 0.25_f32.sin_cos();
        let active = TestTrack::new(7)
            .rotation_format(format)
            .rotations(&[(0, IDENTITY), (9, [sine, 0.0, 0.0, cosine])]);
        assert_eq!(
            classification(active)?,
            Some(false),
            "one active key prevents neutral admission: {format:?}"
        );
    }
    Ok(())
}

#[test]
fn absent_rotation_stays_absent_and_antipodal_identity_is_supported() -> TestResult {
    let absent = TestTrack::new(7).positions(&[(0, [0.0; 3])]);
    assert_eq!(
        classification(absent)?,
        None,
        "absence is not an authored identity value"
    );
    let opposite = TestTrack::new(7)
        .rotation_format(TestRotationFormat::Float)
        .rotations(&[(0, [-0.0, -0.0, -0.0, -1.0])]);
    assert_eq!(
        classification(opposite)?,
        Some(true),
        "both signs of a unit identity quaternion represent the same rotation"
    );
    let nonunit = TestTrack::new(7)
        .rotation_format(TestRotationFormat::Float)
        .rotations(&[(0, [0.0, 0.0, 0.0, 2.0])]);
    assert_eq!(
        classification(nonunit)?,
        Some(false),
        "normalization cannot manufacture a canonical encoded identity"
    );
    Ok(())
}

#[test]
fn the_identity_predicate_preserves_canonical_decoder_errors() -> TestResult {
    let track = TestTrack::new(7)
        .rotation_code(0x7f)
        .rotations(&[(0, IDENTITY)]);
    let raw = TestAnimation::new("synthetic.caf", 30).track(track).caf();
    let file = ChunkFile::parse(raw.as_slice())?;
    let source = AnimationSource::read_caf("synthetic.caf", &file)?;
    let controller = source
        .controllers()?
        .next()
        .ok_or("one original controller")?;
    assert!(
        matches!(
            controller.rotation_identity(),
            Err(CorruptAnimation::UnknownFormat {
                kind: "rotation",
                code: 0x7f
            })
        ),
        "an unsupported encoding cannot be defaulted"
    );
    Ok(())
}

#[test]
fn identity_evidence_retains_the_actual_decoded_source_intervals() -> TestResult {
    let raw = TestAnimation::new("synthetic.caf", 30)
        .track(TestTrack::new(7).rotations(&[(0, IDENTITY), (9, IDENTITY)]))
        .caf();
    let file = ChunkFile::parse(raw.as_slice())?;
    let source = AnimationSource::read_caf("synthetic.caf", &file)?;
    let controller = source
        .controllers()?
        .next()
        .ok_or("one original controller")?;
    let expected = controller
        .decode_rotation()?
        .ok_or("authored rotation channel")?;
    let observed = controller
        .rotation_identity()?
        .ok_or("source identity evidence")?;
    assert!(observed.is_canonical(), "actual supported identity bytes");
    assert_eq!(
        observed.frame_range(),
        expected.frame_range,
        "original frame bytes remain paired with the verdict"
    );
    assert_eq!(
        observed.value_range(),
        expected.value_range,
        "original value bytes remain paired with the verdict"
    );
    Ok(())
}
