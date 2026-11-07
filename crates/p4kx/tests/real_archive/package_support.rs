// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Shared package verification without formatting source-bearing failures.

use std::ffi::OsStr;

use crate::package_fault::{self, Check, Fault};
use crate::{media_fixtures, package_measures, package_process};

pub(super) use crate::package_measures::nodes_of;
pub(super) use crate::package_scratch::Scratch;
pub(super) use crate::package_types::{Built, Measured};
pub(super) type TestResult = package_fault::Result<()>;

pub(super) fn build(
    scratch: &Scratch,
    command: &str,
    entity: Option<&str>,
    name: &str,
    extra: &[&OsStr],
) -> package_fault::Result<Built> {
    package_process::build(scratch, command, entity, name, extra)
        .map_err(|source| Fault::boxed(Check::Build, source))
}

pub(super) fn verify(built: &Built) -> package_fault::Result<Measured> {
    package_measures::verify(built).map_err(|source| Fault::boxed(Check::Verify, source))
}

pub(super) fn run(test: &str, verify: impl FnOnce(&Scratch) -> TestResult) {
    media_fixtures::run(|| {
        let scratch = Scratch::new(test)?;
        let result = verify(&scratch);
        package_fault::report(scratch.finish(result))
    });
}
