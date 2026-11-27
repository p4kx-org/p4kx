// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use super::{RegistryFault, Stage};

#[test]
fn registry_process_status_preserves_the_stage_and_native_code() {
    for (stage, code, expected) in [
        (Stage::Begin, 0, Some(0x4100_0000)),
        (Stage::WriteState, 5, Some(0x4800_0005)),
        (Stage::Commit, 6704, Some(0x4a00_1a30)),
        (Stage::Commit, 0x00ff_ffff, Some(0x4aff_ffff)),
        (Stage::Commit, 0x0100_0000, None),
        (Stage::Begin, -1, None),
    ] {
        let error = RegistryFault::Io {
            stage,
            source: io::Error::from_raw_os_error(code),
        };
        assert_eq!(
            error.process_status(),
            expected,
            "registry diagnostic for {stage:?} with native code {code}"
        );
    }
    assert_eq!(
        RegistryFault::Conflict.process_status(),
        None,
        "a comparison conflict has no native code"
    );
}
