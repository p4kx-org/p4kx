// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_chunks::{ChunkFile, ivo};
use p4kx_testkit::{TestChunkFile, TestSkeleton};

use crate::PositionedSkeleton;

#[test]
fn positioned_skeletons_keep_each_original_layout_and_name_slot() {
    let original = TestSkeleton::two_bones();
    let file = TestChunkFile::ivo()
        .chunk(ivo::COMPILED_BONES.id(), 0x901, original.compiled_bones())
        .chunk(
            ivo::COMPILED_BONES_2.id(),
            0x900,
            original.compiled_bones_2(),
        )
        .chunk(
            ivo::COMPILED_BONES_2.id(),
            0x901,
            original.compiled_bones_2(),
        );
    let file = ChunkFile::parse(file.bytes()).expect("IVO fixture");
    let check = |ordinal, name_at, parent_at, relative_at, world_at, node_at: Option<u64>| {
        let chunk = file.chunks().get(ordinal).expect("skeleton chunk");
        let bytes = file.chunk_data(ordinal).expect("complete original span");
        assert!(PositionedSkeleton::supports(chunk));
        let positioned = PositionedSkeleton::read(chunk, ordinal, bytes)
            .expect("valid skeleton")
            .expect("measured layout");
        let [_, tip] = positioned.bones() else {
            panic!("two original bone slots");
        };
        let base = chunk.offset();
        let fields = positioned.sources();
        let source = fields.bone(1).expect("original tip fields");
        assert_eq!(fields.chunk(), ordinal);
        assert_eq!(fields.count_range(), base..base + 4);
        assert_eq!(tip.name, "tip");
        assert_eq!(tip.parent, Some(0));
        assert_eq!(tip.relative.translation, [0.0, 0.0, 2.0]);
        assert_eq!(source.name(), base + name_at..base + name_at + 4);
        assert_eq!(source.parent().start, base + parent_at);
        assert_eq!(
            source.relative_pose(),
            base + relative_at..base + relative_at + 28
        );
        assert_eq!(source.world_pose(), base + world_at..base + world_at + 28);
        assert_eq!(
            source.node().map(|range| range.start),
            node_at.map(|at| base + at)
        );
    };
    check(0, 85, 72, 117, 173, Some(78));
    check(1, 145, 80, 84, 112, None::<u64>);
    let future = file.chunks().get(2).expect("future chunk");
    assert!(!PositionedSkeleton::supports(future));
    assert!(
        PositionedSkeleton::read(future, 2, &[])
            .expect("unsupported layout")
            .is_none()
    );
}

#[test]
fn positioned_skeleton_requires_its_original_extent_and_finite_pose() {
    let mut data = TestSkeleton::two_bones().compiled_bones_2();
    let rotation = data.get_mut(16..20).expect("first rotation word");
    rotation.copy_from_slice(&f32::NAN.to_le_bytes());
    let file = TestChunkFile::ivo().chunk(ivo::COMPILED_BONES_2.id(), 0x900, data);
    let file = ChunkFile::parse(file.bytes()).expect("IVO fixture");
    let chunk = file.chunks().first().expect("skeleton chunk");
    let bytes = file.chunk_data(0).expect("complete original span");
    assert!(PositionedSkeleton::read(chunk, 0, &[]).is_err());
    assert!(PositionedSkeleton::read(chunk, 0, bytes).is_err());
}
