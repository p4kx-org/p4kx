// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::sdk_full_proof as full_proof;
use p4kx_export::CatalogError;
use p4kx_export::catalog_queries::equipment::weapon_animation_sources::{
    MannequinBranchVisit, MannequinFragmentStatus, WeaponAnimationSourcesSnapshot,
};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, DatasetAvailability,
};
use p4kx_extraction::{CancelToken, SourceValueState};
use p4kx_testkit::TestEntry;
use serde_json::{Value, json};

use super::fixture::{database, database_without_weapon, documents};
use super::{Fixture, Input, TestResult, origin};

const GRAPH: &str = "/definitions/weaponAnimationSources";

#[test]
fn reached_weapon_animation_graph_matches_canonical_sources_and_origins() -> TestResult {
    let data = database();
    for input in [Input::Plain, Input::Encrypted] {
        let fixture = Fixture::open(&data, input, documents())?;
        let (_, catalog) = fixture.diagnostic()?;
        full_proof::from_entry(
            &fixture.archive,
            &fixture.entry,
            full_proof::NO_LANGUAGES,
            None,
        )?
        .with_player_weapon_animation_sources(None, |snapshot| -> TestResult {
            assert_authored_graph(&snapshot, input)?;
            let query = json!({
                "definitions": {"weaponAnimationSources": snapshot.sources},
                "provenance": snapshot.source_inventory,
            });
            let actual = origin::with_origins(origin::at(&query, GRAPH)?.clone(), &query)?;
            let expected = origin::with_origins(origin::at(&catalog, GRAPH)?.clone(), &catalog)?;
            assert_eq!(
                actual, expected,
                "all eight paths retain original ordering, values and physical use facts"
            );
            origin::closure(&query)
        })??;
    }
    Ok(())
}

fn assert_authored_graph(
    snapshot: &WeaponAnimationSourcesSnapshot<'_>,
    input: Input,
) -> TestResult {
    assert!(
        matches!(snapshot.availability, DatasetAvailability::Available),
        "the fixture declares physical Player motion sources"
    );
    assert_eq!(
        snapshot.dependency_coverage.failed_entries(),
        [],
        "every reached Archive Entry in the fixture is readable"
    );
    let graph = &snapshot.sources;
    let player = graph.player.as_ref().ok_or("Player source selection")?;
    assert_eq!(
        player.state(),
        SourceValueState::Present,
        "original Player is selected"
    );
    assert_eq!(
        player.origin().contains_encrypted_source(),
        input.marker(),
        "the Player Record retains its own source encryption marker"
    );
    let [binding] = graph.actor_bindings.as_slice() else {
        return Err("one physical Player animation binding".into());
    };
    assert_eq!(
        binding.record.value(),
        Some(&0),
        "original physical Player Record"
    );
    assert_eq!(
        binding.component.value(),
        Some(&0),
        "original Player component position"
    );
    let controller = graph.controllers.first().ok_or("original controller")?;
    let target = binding
        .controller
        .source
        .as_ref()
        .ok_or("controller target")?;
    assert_eq!(
        controller.source.entry_index(),
        target.entry_index(),
        "binding targets the physical controller Entry"
    );
    assert_eq!(
        controller.source.input_ref(),
        target.input_ref(),
        "binding and controller share the original Archive input"
    );
    assert_reached_selectors(snapshot)
}

fn assert_reached_selectors(snapshot: &WeaponAnimationSourcesSnapshot<'_>) -> TestResult {
    let graph = &snapshot.sources;
    assert_eq!(
        snapshot.graph_coverage.cyclic_branches, 1,
        "one authored cycle is reported"
    );
    assert_eq!(
        snapshot.graph_coverage.unresolved_branches, 1,
        "one original selector has an unresolved target"
    );
    assert!(
        graph.branches.iter().any(|branch| {
            matches!(branch.traversal.value(), Some(MannequinBranchVisit::Cycle))
        }),
        "the sampled branch retains its own cycle outcome"
    );
    assert!(
        graph.fragment_selectors.iter().any(|selector| {
            matches!(
                selector.status.value(),
                Some(MannequinFragmentStatus::DifferentScope)
            )
        }),
        "separate definition scopes distinguish the same authored tag name"
    );
    assert_eq!(
        graph.scopes.len(),
        3,
        "all three physical database scopes are represented"
    );
    let scope = graph.scopes.first().ok_or("physical database scope")?;
    assert_eq!(
        scope.tags.declarations().count(),
        1,
        "one original tag declaration"
    );
    assert_eq!(
        scope.tags.imports().len(),
        1,
        "one authored definition import"
    );
    Ok(())
}
#[test]
fn player_graph_reads_without_a_weapon_while_catalog_admission_stays_unchanged() -> TestResult {
    let data = database_without_weapon();
    let mut docs = documents();
    docs.push(TestEntry::stored(
        "Data/Animations/UnreachableBroken.adb",
        b"<broken>",
    ));
    let fixture = Fixture::open(&data, Input::Plain, docs)?;
    let (_, catalog) = fixture.diagnostic()?;
    let graph = origin::at(&catalog, GRAPH)?;
    assert_eq!(
        graph["player"],
        Value::Null,
        "the writer gates the graph on weapons"
    );
    assert_eq!(
        graph["actorBindings"],
        json!([]),
        "the gated Catalog has no Player binding rows"
    );
    let session = CatalogQuerySession::from_entry(
        &fixture.archive,
        &fixture.entry,
        CatalogLanguageSelection::Selected(&[]),
        None,
    )?;
    let parsed = session.parsed(None)?;
    parsed.with_player_weapon_animation_sources(None, |snapshot| {
        assert_authored_graph(&snapshot, Input::Plain)
    })??;
    let cancel = CancelToken::default();
    cancel.cancel();
    assert!(
        matches!(
            parsed.with_player_weapon_animation_sources(Some(&cancel), |_| ()),
            Err(CatalogError::Cancelled)
        ),
        "pre-cancelled queries do not expose a graph"
    );
    Ok(())
}
