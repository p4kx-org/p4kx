// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::validate_flatpak_info;

#[test]
fn sandbox_evidence_requires_application_and_instance_groups() {
    validate_flatpak_info(
        b"[Application]\nname=io.github.p4kx_org.p4kx\n[Instance]\ninstance-id=42\n",
    )
    .expect("actual sandbox metadata shape");
    for invalid in [
        b"".as_slice(),
        b"FLATPAK_ID=io.github.p4kx_org.p4kx",
        b"[Application]\nname=io.github.p4kx_org.p4kx\n",
        b"[Application]\nname=\n[Instance]\n",
        b"[Application]\nname=a\nname=b\n[Instance]\n",
        b"[Application]\nname=\xff\n[Instance]\n",
    ] {
        assert!(
            validate_flatpak_info(invalid).is_err(),
            "invalid sandbox evidence"
        );
    }
}
