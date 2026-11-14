// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Catalog upload-size warning boundaries through the public API.

use p4kx_export::SizeWarning;

#[test]
fn the_size_warning_comes_before_the_64_mb_upload_limit() {
    let limit = 64_000_000;
    assert_eq!(SizeWarning::for_size(1_000_000), None, "small");
    assert_eq!(SizeWarning::for_size(57_599_999), None, "below 90 %");
    assert_eq!(
        SizeWarning::for_size(57_600_000),
        Some(SizeWarning::NearLimit),
        "from 90 %"
    );
    assert_eq!(
        SizeWarning::for_size(limit),
        Some(SizeWarning::NearLimit),
        "at the limit"
    );
    assert_eq!(
        SizeWarning::for_size(limit + 1),
        Some(SizeWarning::OverLimit),
        "over the limit"
    );
}
