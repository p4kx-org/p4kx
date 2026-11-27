// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Error, ErrorKind};

#[test]
fn typed_original_errors_remain_internal_to_static_package_failures() -> super::Result<()> {
    let error = super::Fault::at(super::Check::Verify, Error::other("PRIVATE_SENTINEL"));
    let fault = error
        .downcast_ref::<super::Fault>()
        .ok_or("typed failure missing")?;
    let source = fault
        .original()
        .and_then(|source| source.downcast_ref::<Error>())
        .ok_or("original typed cause missing")?;
    assert_eq!(
        source.kind(),
        ErrorKind::Other,
        "the original producer cause remains inspectable internally"
    );
    assert_eq!(
        source.to_string(),
        "PRIVATE_SENTINEL",
        "the original payload remains inspectable through the explicit internal accessor"
    );
    assert!(
        !format!("{error:?} {error}").contains("PRIVATE_SENTINEL"),
        "failure Debug/Display contain only the static check category"
    );
    assert!(
        error.source().is_none(),
        "the original cause cannot leak through generic chain formatting"
    );
    Ok(())
}
