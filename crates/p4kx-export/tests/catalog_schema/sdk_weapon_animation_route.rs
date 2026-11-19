// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment::with_weapon_animation_candidates;
use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use super::super::{Route, TestResult, WEAPON_ANIMATION};

#[expect(
    dead_code,
    reason = "The schema witness selects only the fixture personal weapon."
)]
#[path = "../it/weapon_player_animation_fixture.rs"]
mod fixture;

pub(super) fn read() -> TestResult<Route> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("sdk_weapon_animation.p4k");
    let mut archive =
        ArchiveBuilder::new().entry(TestEntry::stored("Data\\Game2.dcb", fixture::database()));
    for entry in fixture::documents() {
        archive = archive.entry(entry);
    }
    archive.write_to(&path)?;
    let archive = Archive::open(path)?;
    with_weapon_animation_candidates(&archive, fixture::WEAPON, None, |result| {
        if result.rows.len() != 2
            || result
                .rows
                .iter()
                .any(|row| row.weapon_record_index().value() != Some(&fixture::WEAPON))
        {
            return Err(
                "the query witness must retain both physical options of the selected weapon".into(),
            );
        }
        Route::queried_rows(
            WEAPON_ANIMATION,
            &result.rows,
            "physical Item query available; installed producer parity and owner enumeration open",
        )
    })?
}
