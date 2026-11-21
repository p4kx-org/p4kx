// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment::weapon_animation_sources::with_player_weapon_animation_sources;
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

pub(super) const GRAPH_DATASETS: [&str; 8] = [
    "/definitions/weaponAnimationSources/player",
    "/definitions/weaponAnimationSources/actorBindings",
    "/definitions/weaponAnimationSources/controllers",
    "/definitions/weaponAnimationSources/databases",
    "/definitions/weaponAnimationSources/tagDefinitions",
    "/definitions/weaponAnimationSources/scopes",
    "/definitions/weaponAnimationSources/branches",
    "/definitions/weaponAnimationSources/fragmentSelectors",
];

const REMAINING: &str =
    "synthetic producer parity witnessed; installed graph parity and memory evidence open";

pub(super) fn read() -> TestResult<Vec<Route>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("sdk_weapon_animation.p4k");
    let mut archive =
        ArchiveBuilder::new().entry(TestEntry::stored("Data\\Game2.dcb", fixture::database()));
    for entry in fixture::documents() {
        archive = archive.entry(entry);
    }
    archive.write_to(&path)?;
    let archive = Archive::open(path)?;
    let candidate = with_weapon_animation_candidates(&archive, fixture::WEAPON, None, |result| {
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
    })??;
    let mut routes = with_player_weapon_animation_sources(&archive, None, |result| {
        let graph = result.sources;
        if graph
            .player
            .as_ref()
            .and_then(|player| player.value())
            .is_none_or(Vec::is_empty)
        {
            return Err("the query witness needs a physical Player Record".into());
        }
        let [
            player,
            bindings,
            controllers,
            databases,
            definitions,
            scopes,
            branches,
            fragments,
        ] = GRAPH_DATASETS;
        Ok::<_, Box<dyn std::error::Error>>(vec![
            Route::queried_value(player, &graph.player, REMAINING)?,
            Route::queried_rows(bindings, &graph.actor_bindings, REMAINING)?,
            Route::queried_rows(controllers, &graph.controllers, REMAINING)?,
            Route::queried_rows(databases, &graph.databases, REMAINING)?,
            Route::queried_rows(definitions, &graph.tag_definitions, REMAINING)?,
            Route::queried_rows(scopes, &graph.scopes, REMAINING)?,
            Route::queried_rows(branches, &graph.branches, REMAINING)?,
            Route::queried_rows(fragments, &graph.fragment_selectors, REMAINING)?,
        ])
    })??;
    routes.push(candidate);
    Ok(routes)
}
