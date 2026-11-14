// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::MAX_PART_BYTES;

use crate::ArtifactExtension;

#[test]
fn encoding_suffixes_obey_the_actual_portable_filename_limit() {
    const PREFIX_BYTES: usize = 64 + 1;
    for value in ["png", "exr", "glb", "ogg", "m4a", "ktx2", "con"] {
        let extension = ArtifactExtension::new(value).expect("canonical suffix");
        assert_eq!(extension.as_str(), value);
        p4kx_extraction::check_part(&format!("{}.{}", "a".repeat(64), extension.as_str()))
            .expect("portable complete name");
    }
    let longest = "a".repeat(MAX_PART_BYTES - PREFIX_BYTES);
    assert!(ArtifactExtension::new(&longest).is_ok());
    assert!(ArtifactExtension::new(&(longest + "a")).is_err());
    for value in [
        "", "PNG", ".png", "../png", "a/b", "a\\b", "a:b", "a.", "a ", "a-b", "\u{fc}", "\0",
    ] {
        assert!(
            ArtifactExtension::new(value).is_err(),
            "invalid suffix {value:?}"
        );
    }
}
