// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io::Write as _;

use same_file::Handle;

#[test]
fn precommit_context_describes_the_staged_descriptor_that_is_renamed() {
    let dir = tempfile::tempdir().expect("folder");
    let out = dir.path().join("artifact.bin");
    fs::write(&out, b"previous").expect("previous output");
    let ((), staged) = super::write_replacing_protected_before(
        &out,
        None,
        None,
        |writer| {
            writer
                .write_all(b"current")
                .map_err(|error| super::write_error(&out, error))
        },
        |temporary| {
            assert_eq!(temporary.parent(), out.parent());
            assert_eq!(fs::read(&out).expect("previous output"), b"previous");
            assert_eq!(fs::read(temporary).expect("staged output"), b"current");
            Handle::from_path(temporary).map_err(|error| super::write_error(temporary, error))
        },
    )
    .expect("atomic replacement");
    assert_eq!(
        staged,
        Handle::from_path(&out).expect("committed descriptor")
    );
}
