// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::Path;

use p4kx_testkit::{
    HEAD_REGIONS, HEAD_VERTICES, MALE_RIG_HASH, TestAnimation, TestChf, TestEntry, TestSkeleton,
    TestTrack, head_entries, head_rig,
};

const SOURCE_RATE: u16 = 30;
const PELVIS: usize = 0;
const SELECTED_FACE_REGION: usize = 3;

pub(super) fn entries() -> Vec<TestEntry> {
    let rig = head_rig(HEAD_VERTICES, MALE_RIG_HASH).build();
    let translation = TestTrack::new(TestSkeleton::controller_id(PELVIS)).positions(&[
        (0, [0.0, 0.0, 0.0]),
        (9, [1.5, -0.25, 0.75]),
        (30, [-0.5, 0.5, 0.0]),
    ]);
    let clip = TestAnimation::new("absolute_stride.caf", SOURCE_RATE).track(translation);
    let mut entries = head_entries(Some(rig));
    entries.push(TestEntry::stored(
        r"Data\Animations\Characters\Human\male_v7\absolute_stride.caf",
        clip.caf(),
    ));
    entries
}

pub(super) fn write_chf(folder: &Path) -> io::Result<()> {
    let mut face_parts = vec![vec![(0, 0); 2]; HEAD_REGIONS];
    let part = face_parts
        .get_mut(SELECTED_FACE_REGION)
        .and_then(|region| region.first_mut())
        .ok_or_else(|| io::Error::other("missing face region in the character fixture"))?;
    *part = (u16::MAX, 2);
    let chf = TestChf {
        dna_hashes: Some((MALE_RIG_HASH, 0)),
        face_parts,
        ..TestChf::v8()
    };
    std::fs::write(folder.join("hero.chf"), chf.to_bytes())
}
