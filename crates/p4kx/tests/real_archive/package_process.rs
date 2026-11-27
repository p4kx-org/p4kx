// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::ffi::OsStr;
use std::fmt;
use std::path::Path;
use std::process::Output;
use std::time::Instant;

use serde_json::Value;

use crate::package_fault::{self, Check, Fault};
use crate::package_scratch::Scratch;
use crate::package_types::Built;

struct FailedProcess(Output);

impl fmt::Debug for FailedProcess {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            output,
            "package_subprocess_failed({:?})",
            self.0.status.code()
        )
    }
}
impl fmt::Display for FailedProcess {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, output)
    }
}
impl Error for FailedProcess {}

pub(crate) fn build(
    scratch: &Scratch,
    command: &str,
    entity: Option<&str>,
    name: &str,
    extra: &[&OsStr],
) -> package_fault::Result<Built> {
    let out = scratch.path().join(name);
    let mut run = p4kx_testkit::p4kx()?;
    run.env(
        "P4KX_ARCHIVE_KEY",
        p4kx_testkit::real_archive_key()?.to_hex(),
    );
    run.args(["--json", command, "package"])
        .arg(p4kx_testkit::real_archive_path()?);
    if let Some(entity) = entity {
        run.arg(entity);
    }
    let started = Instant::now();
    let output = run.arg("--out").arg(&out).args(extra).output()?;
    let elapsed = started.elapsed();
    let (report, manifest) = decode(output, &out)?;
    Ok(Built {
        out,
        report,
        manifest,
        elapsed,
    })
}

fn decode(output: Output, out: &Path) -> package_fault::Result<(Value, Value)> {
    if !output.status.success() {
        return Err(Fault::at(Check::ProcessExit, FailedProcess(output)));
    }
    let report = serde_json::from_slice(&output.stdout)?;
    let manifest = serde_json::from_slice(&std::fs::read(out.join("scene.json"))?)?;
    Ok((report, manifest))
}

#[cfg(test)]
#[path = "package_process_tests.rs"]
mod tests;
