// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Tests the shared installation guard with a configured bottle.

use std::error::Error;
use std::fs;
use std::sync::Arc;

use p4kx_channels::{SearchPaths, SearchRecipe};
use p4kx_guard::{
    WriteProtectionError, configure_library_discovery, ensure_writable, protect_libraries_report,
};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn configured_bottle_sources_refresh_the_guard_and_keep_prior_installations_protected() -> TestResult
{
    let temporary = tempfile::tempdir()?;
    let root = temporary.path().join("Configured Bottles");
    let bottle = root.join("Synthetic");
    let default_library = bottle.join("drive_c/Program Files/Roberts Space Industries/StarCitizen");
    let custom_library = bottle.join("drive_c/Custom/StarCitizen");
    let logs = bottle.join("drive_c/users/Pilot/AppData/Roaming/rsilauncher/logs");
    for path in [&default_library, &custom_library, &logs] {
        fs::create_dir_all(path)?;
    }
    let recipe = Arc::new(
        SearchRecipe::explicit(SearchPaths::default()).with_wine_locations(Vec::new(), vec![root]),
    );
    configure_library_discovery(recipe);
    assert!(
        protect_libraries_report().is_empty(),
        "the complete configured bottle starts without a launcher log"
    );
    assert!(
        ensure_writable(&default_library.join("new-output")).is_err(),
        "the configured bottle's default library is protected"
    );
    ensure_writable(&custom_library.join("new-output"))?;

    let current = logs.join("log.log");
    fs::write(
        &current,
        "{ \"message\": \"Installing Star Citizen PTU at C:\\\\Custom\\\\StarCitizen (force DP false)\" }\n",
    )?;
    assert!(
        protect_libraries_report().is_empty(),
        "the same configured source sees a newly created launcher log"
    );
    assert!(
        matches!(
            ensure_writable(&custom_library.join("new-output")),
            Err(WriteProtectionError::Installation(_))
        ),
        "the logged library is protected by its actual mapped drive path"
    );

    fs::remove_file(current)?;
    assert!(
        protect_libraries_report().is_empty(),
        "a disappeared log is normal absence"
    );
    assert!(
        ensure_writable(&custom_library.join("new-output")).is_err(),
        "removing a source does not remove previously established write protection"
    );
    Ok(())
}
