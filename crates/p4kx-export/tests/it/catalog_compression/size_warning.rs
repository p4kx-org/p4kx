// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::SizeWarning;

#[test]
fn the_decompressed_size_is_checked_against_1_gb_and_the_file_against_64_mb() {
    assert_eq!(SizeWarning::for_decompressed_size(26_500_000), None, "now");
    assert_eq!(
        SizeWarning::for_decompressed_size(900_000_000),
        Some(SizeWarning::NearLimit),
        "from 90 %"
    );
    assert_eq!(
        SizeWarning::for_decompressed_size(1_000_000_001),
        Some(SizeWarning::OverLimit),
        "over 1 GB"
    );
    assert_eq!(SizeWarning::for_size(1_700_000), None, "compressed LIVE");
}
