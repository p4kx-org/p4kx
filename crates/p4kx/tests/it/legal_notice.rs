// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The legal notice in `p4kx --version` and `p4kx --help`.

use std::error::Error;

use predicates::prelude::predicate;

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn both_version_flags_name_the_fan_project_and_the_users_responsibility() -> TestResult {
    for flag in ["--version", "-V"] {
        p4kx_testkit::p4kx()?
            .arg(flag)
            .assert()
            .success()
            .stdout(predicate::str::contains(
                "Unofficial Star Citizen fan project",
            ))
            .stdout(predicate::str::contains("Terms of Service"))
            .stdout(predicate::str::contains(
                "not affiliated with or endorsed by the Cloud Imperium group",
            ));
    }
    Ok(())
}

#[test]
fn help_names_the_fan_project_and_the_users_responsibility() -> TestResult {
    p4kx_testkit::p4kx()?
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "not affiliated with or endorsed by the Cloud Imperium group",
        ))
        .stdout(predicate::str::contains("Ships no game data"))
        .stdout(predicate::str::contains("reads only your own installation"))
        .stdout(predicate::str::contains("EULA and Terms of Service"))
        .stdout(predicate::str::contains("check-update"));
    Ok(())
}
