// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
};
use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use super::super::{Route, TestResult};

#[path = "../it/catalog_weapon_procedural_sdk_fixture.rs"]
mod fixture;

const ANIMATIONS: &str = "/definitions/weaponProceduralAnimations";
const CLIPS: &str = "/definitions/weaponProceduralClips";
pub(super) const DATASETS: [&str; 2] = [ANIMATIONS, CLIPS];

const REMAINING: &str =
    "synthetic producer parity witnessed; installed parity and memory evidence open";

pub(super) fn read() -> TestResult<[Route; 2]> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("sdk_weapon_procedural.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data/Game2.dcb",
            fixture::database().build(),
        ))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let parsed = session.parsed(None)?;
    let animations = parsed.with_weapon_procedural_animations(
        CatalogRecordSelection::All,
        None,
        |snapshot| {
            let [first, alias] = snapshot.rows() else {
                return Err("the procedural animation witness preserves both physical rows".into());
            };
            assert_eq!(
                [
                    first.source().record_index().value(),
                    alias.source().record_index().value()
                ],
                [
                    Some(&fixture::ANIMATION_INDEX),
                    Some(&fixture::ANIMATION_ALIAS_INDEX)
                ],
                "both physical animation rows keep the original Record order"
            );
            Route::queried_rows(ANIMATIONS, snapshot.rows(), REMAINING)
        },
    )??;
    let clips = parsed.with_weapon_procedural_clips(
        CatalogRecordSelection::RecordIndex(fixture::CLIP_INDEX),
        None,
        |snapshot| {
            let [row] = snapshot.rows() else {
                return Err("the procedural clip witness preserves a selected physical row".into());
            };
            assert_eq!(
                row.source().record_index().value(),
                Some(&fixture::CLIP_INDEX),
                "the scoped witness returns the selected physical clip"
            );
            Route::queried_rows(CLIPS, snapshot.rows(), REMAINING)
        },
    )??;
    Ok([animations, clips])
}
