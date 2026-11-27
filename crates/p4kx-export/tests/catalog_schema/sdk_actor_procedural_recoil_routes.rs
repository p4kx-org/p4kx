// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
};
use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use super::super::{Route, TestResult};

use super::weapon_operating_fixture as fixture;

pub(super) const DATASETS: [&str; 2] = [
    "/definitions/actorProceduralRecoilConfigs",
    "/definitions/actorProceduralRecoilModifiers",
];

pub(super) fn read() -> TestResult<[Route; 2]> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("sdk_actor_recoil.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data/Game2.dcb",
            fixture::database_with_actor_recoil().build(),
        ))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let parsed = session.parsed(None)?;
    let selection = CatalogRecordSelection::All;
    let configs = parsed.with_actor_procedural_recoil_configs(selection, None, |rows| {
        let [original, alias] = rows.rows() else {
            return Err("two physical actor recoil configurations".into());
        };
        assert_eq!(
            [
                original.source().record_index().value(),
                alias.source().record_index().value()
            ],
            [
                Some(&fixture::ACTOR_RECOIL_INDEX),
                Some(&fixture::ACTOR_RECOIL_ALIAS_INDEX)
            ],
            "the schema witness retains both physical configuration occurrences"
        );
        Route::queried_rows(DATASETS[0], rows.rows(), None)
    })??;
    let modifiers = parsed.with_actor_procedural_recoil_modifiers(selection, None, |rows| {
        let [row] = rows.rows() else {
            return Err("one physical actor recoil modifier".into());
        };
        assert_eq!(
            row.source().record_index().value(),
            Some(&fixture::ACTOR_MODIFIER_INDEX),
            "the schema witness reads the original modifier Record"
        );
        Route::queried_rows(DATASETS[1], rows.rows(), None)
    })??;
    Ok([configs, modifiers])
}
