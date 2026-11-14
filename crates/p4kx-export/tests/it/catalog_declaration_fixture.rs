// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogPublicationMode, build_catalog};
use serde_json::Value;

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};

pub(crate) fn diagnostic_bytes(bytes: Vec<u8>) -> TestResult<Value> {
    let (_folder, archive) = common::open_bytes(bytes, DcbOrigin::Plain)?;
    let mut options = common::options();
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    Ok(serde_json::from_slice(
        build_catalog(&archive, &options)?.json(),
    )?)
}

pub(crate) fn corrupt_definition_name(bytes: &mut [u8], name: &str) -> TestResult {
    let name = name.as_bytes();
    let mut positions = bytes
        .windows(name.len() + 1)
        .enumerate()
        .filter_map(|(index, word)| (word.strip_suffix(&[0]) == Some(name)).then_some(index));
    let index = positions.next().ok_or("fixture definition name")?;
    assert!(
        positions.next().is_none(),
        "one physical fixture definition name"
    );
    *bytes.get_mut(index).ok_or("definition name byte")? = u8::MAX;
    Ok(())
}
