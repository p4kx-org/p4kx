// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{TestAnimation, TestDba, TestSkeleton, TestTimes, TestTrack};

use crate::support::TestResult;

const IDENTITY: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

pub(super) const DBA_SELECTED: &str = "clip/selected.caf";

pub(super) struct DatabaseFixture {
    pub(super) bytes: Vec<u8>,
    pub(super) name: usize,
    pub(super) bitmap: usize,
}

pub(super) fn dba_fixture() -> TestResult<DatabaseFixture> {
    let skipped = "clip/unread.caf";
    let selected = DBA_SELECTED;
    let mut dba = TestDba::new()
        .animation(
            TestAnimation::new(skipped, 30).track(
                TestTrack::new(1)
                    .rotations(&[(0, IDENTITY)])
                    .rotation_code(0xFF),
            ),
        )
        .animation(
            TestAnimation::new(selected, 20).track(
                TestTrack::new(TestSkeleton::controller_id(1))
                    .times(TestTimes::Bitmap)
                    .rotations(&[(0, IDENTITY), (20, IDENTITY)]),
            ),
        )
        .pool_after_metadata()
        .bytes();
    let first_name = find_bytes(&dba, skipped.as_bytes())?;
    let name = find_bytes(&dba, selected.as_bytes())?;
    let timing = first_name - 48;
    set_end(&mut dba, timing, 5)?;
    let bitmap = find_bytes(&dba, &[0, 0, 20, 0, 1, 0, 16])?;
    Ok(DatabaseFixture {
        bytes: dba,
        name,
        bitmap,
    })
}

pub(super) fn set_end(bytes: &mut [u8], record: usize, frame: u32) -> TestResult {
    let target = bytes
        .get_mut(record + 16..record + 20)
        .ok_or("authored end frame word")?;
    target.copy_from_slice(&frame.to_le_bytes());
    Ok(())
}

fn find_bytes(bytes: &[u8], pattern: &[u8]) -> TestResult<usize> {
    let mut matches = bytes
        .windows(pattern.len())
        .enumerate()
        .filter(|(_, window)| *window == pattern);
    let (first, _) = matches.next().ok_or("original byte pattern")?;
    assert!(matches.next().is_none(), "source pattern is unique");
    Ok(first)
}
