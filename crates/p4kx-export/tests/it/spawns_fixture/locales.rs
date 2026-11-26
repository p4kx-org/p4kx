// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Language;
use p4kx_testkit::{GlobalIniBuilder, TestEntry};

pub(super) fn english() -> TestEntry {
    TestEntry::zstd(
        Language::English.global_ini_path(),
        GlobalIniBuilder::new()
            .text("place_name", "Hijacked Ship")
            .build(),
    )
}

pub(super) fn french() -> TestEntry {
    TestEntry::zstd(
        Language::French.global_ini_path(),
        GlobalIniBuilder::new()
            .text("place_name", "Site de mission")
            .build(),
    )
}
