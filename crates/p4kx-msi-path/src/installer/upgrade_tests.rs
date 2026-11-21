// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::installer::lease::Lease;
use crate::installer::raw_path::StringKind;
use crate::installer::state::Request;

use super::{ENTRY, NEXT_PRODUCT, PRODUCT, TestResult};
use crate::installer::tests;

use super::UPGRADED_ENTRY;

#[test]
fn renamed_upgrade_removes_only_the_previous_lease_and_rolls_back_exactly() -> TestResult {
    let original = tests::path(";;A;B;;", StringKind::Expandable);
    let mut store = tests::memory(original.clone());
    tests::install(&mut store)?;
    let old = store.state.path.clone();
    let upgraded = tests::begin(
        &mut store,
        Request::Upgrade {
            from: PRODUCT,
            to: NEXT_PRODUCT,
            lease: Lease::try_new(tests::bytes(UPGRADED_ENTRY))?,
        },
    )?;
    tests::apply(&mut store, upgraded)?;
    assert_eq!(
        store.state.path,
        tests::path(&format!("{UPGRADED_ENTRY};;;A;B;;"), StringKind::Expandable),
        "rename changes only our owned element"
    );
    tests::commit(&mut store, upgraded)?;
    tests::rollback(&mut store, upgraded)?;
    assert_eq!(
        store.state.path, old,
        "later MSI commit failure restores exact previous lease bytes"
    );
    tests::remove(&mut store, PRODUCT)?;
    assert_eq!(
        store.state.path, original,
        "original source bits survive rename and rollback"
    );
    Ok(())
}

#[test]
fn renamed_upgrade_rollback_preserves_concurrent_foreign_segments_and_kind() -> TestResult {
    let original = tests::path("A;B", StringKind::Expandable);
    let mut store = tests::memory(original);
    tests::install(&mut store)?;
    let upgraded = tests::begin(
        &mut store,
        Request::Upgrade {
            from: PRODUCT,
            to: NEXT_PRODUCT,
            lease: Lease::try_new(tests::bytes(UPGRADED_ENTRY))?,
        },
    )?;
    tests::apply(&mut store, upgraded)?;
    store.state.path = tests::path(&format!("Z;;{UPGRADED_ENTRY};A;B;;Q"), StringKind::Plain);
    tests::rollback(&mut store, upgraded)?;
    assert_eq!(
        store.state.path,
        tests::path(&format!("{ENTRY};Z;;A;B;;Q"), StringKind::Plain),
        "compensation retains foreign ordering, empty elements and registry kind"
    );
    tests::remove(&mut store, PRODUCT)?;
    assert_eq!(
        store.state.path,
        tests::path("Z;;A;B;;Q", StringKind::Plain),
        "restored owner removes its own lease only"
    );
    Ok(())
}
