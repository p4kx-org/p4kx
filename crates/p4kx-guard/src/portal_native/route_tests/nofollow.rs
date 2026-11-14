// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::os::unix::fs::symlink;

use crate::portal_alias::AliasUncertainty;

use super::{RouteFixture, TestResult};

#[test]
fn internal_suffix_links_are_refused_through_direct_runtime_and_outer_routes() -> TestResult {
    for destination in ["aabbccdd/exports/real", "deadbeef/exports"] {
        let fixture = RouteFixture::new()?;
        let runtime = fixture.runtime_alias()?;
        fs::create_dir(fixture.mount.join("aabbccdd/exports/real"))?;
        symlink(
            fixture.mount.join(destination),
            fixture.mount.join("aabbccdd/exports/link"),
        )?;
        for input in [
            fixture.mount.join("aabbccdd/exports/link/new/output"),
            runtime.join("aabbccdd/exports/link/new/output"),
            fixture.root.path().join("outer-link/link/new/output"),
        ] {
            let (result, calls, _) = fixture.resolve_path(&input, &runtime, || Ok(()));
            assert_eq!(
                result.unwrap_err().reason(),
                AliasUncertainty::SymbolicLink,
                "the original grant suffix must reach the no-follow snapshot"
            );
            assert_eq!(calls, 0, "an internal link is rejected before host lookup");
        }
    }
    Ok(())
}

#[test]
fn grant_root_links_are_refused_before_their_destination_becomes_a_new_grant() -> TestResult {
    let fixture = RouteFixture::new()?;
    let runtime = fixture.runtime_alias()?;
    symlink(
        fixture.mount.join("aabbccdd/exports"),
        fixture.mount.join("aabbccdd/root-link"),
    )?;
    for base in [&fixture.mount, &runtime] {
        let (result, calls, _) = fixture.resolve_path(
            &base.join("aabbccdd/root-link/new/output"),
            &runtime,
            || Ok(()),
        );
        assert_eq!(
            result.unwrap_err().reason(),
            AliasUncertainty::SymbolicLink,
            "a canonical directory cannot substitute for the exposed grant root"
        );
        assert_eq!(calls, 0);
    }
    Ok(())
}

#[test]
fn deeper_suffix_links_remain_visible_to_the_actual_snapshot() -> TestResult {
    let fixture = RouteFixture::new()?;
    let runtime = fixture.runtime_alias()?;
    fs::create_dir_all(fixture.mount.join("aabbccdd/exports/deeper/real"))?;
    symlink(
        fixture.mount.join("aabbccdd/exports/deeper/real"),
        fixture.mount.join("aabbccdd/exports/deeper/link"),
    )?;
    let (result, calls, _) = fixture.resolve_path(
        &runtime.join("aabbccdd/exports/deeper/link/output"),
        &runtime,
        || Ok(()),
    );
    assert_eq!(result.unwrap_err().reason(), AliasUncertainty::SymbolicLink);
    assert_eq!(calls, 0);
    assert!(
        !fixture
            .mount
            .join("aabbccdd/exports/deeper/real/output")
            .exists()
    );
    Ok(())
}
