// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::Cell;
use std::error::Error;

use p4kx_extraction::{Archive, ArchiveWatch, CancelToken};
use p4kx_testkit::{self, watchdog};
use serde::Serialize;
use serde_json::Value;

use crate::CatalogError;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};

#[path = "installed_sdk_witness_item.rs"]
mod item;
#[path = "installed_loadout_source_witness.rs"]
mod loadout_source;
#[path = "installed_sdk_witness_pool.rs"]
mod pool;
#[path = "installed_sdk_witness_receipt.rs"]
mod receipt;
#[path = "installed_sdk_witness_reward.rs"]
mod reward;
#[path = "installed_sdk_witness_scope.rs"]
mod scope;
#[path = "installed_sdk_witness_selection.rs"]
mod selection;
#[path = "installed_ship_source_witness.rs"]
mod ship_source;
#[path = "installed_sdk_witness_verify.rs"]
mod verify;

use receipt::Report;
use selection::{Census, Family};
use verify::{Context, Evidence, Proof};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

macro_rules! inspect {
    ($context:ident, $snapshot:ident, $tables:expr, $dependencies:expr, $texts:expr) => {
        $context.check(
            Evidence {
                build: $snapshot.build(),
                availability: $snapshot.availability(),
                inventory: $snapshot.source_inventory(),
                coverage: $snapshot.coverage(),
                dependencies: $dependencies,
            },
            &$tables,
            &$texts,
        )
    };
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn installed_selective_sdk_has_original_owners_and_closed_sources() -> TestResult {
    watchdog::start();
    let destination = receipt::destination()?;
    let archive = open_archive()?;
    let watch = ArchiveWatch::new(&archive);
    watch
        .check()
        .map_err(|_| "Archive changed before SDK session open")?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)
            .map_err(|_| "installed SDK session open failed")?;
    let parsed = session
        .parsed(None)
        .map_err(|_| "installed SDK parsed view failed")?;
    binding(&archive, &session, &parsed)?;
    let proof = Proof::read(session.dataforge_origin())?;
    let censuses = selection::DIRECT_FAMILIES
        .into_iter()
        .map(|family| family.census(&parsed.forge))
        .collect::<TestResult<Vec<_>>>()?;
    after_callback_cancellation(&parsed, censuses[0].selection())?;
    let reports = run_all(&archive, &watch, &parsed, &proof, &censuses)?;
    watch
        .check()
        .map_err(|_| "Archive changed before private SDK receipt publication")?;
    receipt::write(&destination, &archive, &reports)
}

fn open_archive() -> TestResult<Archive> {
    let path =
        p4kx_testkit::real_archive_path().map_err(|_| "installed Archive path unavailable")?;
    let key = p4kx_testkit::real_archive_key().map_err(|_| "installed Archive key unavailable")?;
    let archive =
        Archive::open_with_key(path, Some(key)).map_err(|_| "installed Archive open failed")?;
    let build = archive.build().ok_or("installed Build identity missing")?;
    if [
        build.branch(),
        build.version(),
        build.change_number(),
        build.build_id(),
    ]
    .iter()
    .any(|value| value.trim().is_empty())
        || archive.file_modified().is_none()
    {
        return Err("installed Archive requires a complete stable Build identity".into());
    }
    Ok(archive)
}

fn binding(
    archive: &Archive,
    session: &CatalogQuerySession<'_>,
    parsed: &ParsedCatalogQueries<'_>,
) -> TestResult {
    if !session.dataforge_origin().matches_archive(archive)
        || session.entry.get().origin_of(&parsed.forge) != Some(session.dataforge_origin())
        || !std::ptr::eq(parsed.session, session)
        || session.build() != archive.build()
    {
        return Err(
            "session/Entry/parsed DataForge must retain the same original Archive binding".into(),
        );
    }
    Ok(())
}

fn run_all(
    archive: &Archive,
    watch: &ArchiveWatch,
    parsed: &ParsedCatalogQueries<'_>,
    proof: &Proof,
    censuses: &[Census],
) -> TestResult<Vec<Report>> {
    let mut reports = Vec::with_capacity(selection::QUERIES.len());
    for family in selection::QUERIES {
        watch
            .check()
            .map_err(|_| "Archive changed before selective SDK query")?;
        let census = &censuses[family.census_index()];
        cancelled_request(parsed, family, census.selection())?;
        let context = Context {
            archive,
            forge: &parsed.forge,
            proof,
            family,
            census,
        };
        let result = match family {
            Family::Globals | Family::Refining => mining_query(parsed, &context),
            Family::Distributions | Family::Quantizations => quality_query(parsed, &context),
            Family::Groups | Family::Parents => group_query(parsed, &context),
        };
        reports.push(result.map_err(|_| "selective public SDK query outer completion failed")??);
        watch
            .check()
            .map_err(|_| "Archive changed after selective SDK query")?;
    }
    Ok(reports)
}

type QueryResult = Result<TestResult<Report>, CatalogError>;

fn mining_query(parsed: &ParsedCatalogQueries<'_>, context: &Context<'_>) -> QueryResult {
    let selection = context.census.selection();
    match context.family {
        Family::Globals => parsed.with_mining_global_params(selection, None, |snapshot| {
            let texts = text_context(
                snapshot.requested_languages().is_empty(),
                snapshot.available_languages().is_empty(),
                snapshot.localization_notes().is_empty(),
                snapshot.public_names(),
                snapshot.game_texts(),
            )?;
            inspect!(
                context,
                snapshot,
                [wire(snapshot.rows())?],
                Some(snapshot.dependency_coverage()),
                texts
            )
        }),
        Family::Refining => parsed.with_refining_processes(selection, None, |snapshot| {
            let texts = text_context(
                snapshot.requested_languages().is_empty(),
                snapshot.available_languages().is_empty(),
                snapshot.localization_notes().is_empty(),
                snapshot.public_names(),
                snapshot.game_texts(),
            )?;
            inspect!(
                context,
                snapshot,
                [wire(snapshot.rows())?],
                Some(snapshot.dependency_coverage()),
                texts
            )
        }),
        _ => Ok(Err("unexpected mining witness dispatch".into())),
    }
}

fn quality_query(parsed: &ParsedCatalogQueries<'_>, context: &Context<'_>) -> QueryResult {
    let selection = context.census.selection();
    match context.family {
        Family::Distributions => parsed.with_quality_distributions(selection, None, |snapshot| {
            inspect!(context, snapshot, [wire(snapshot.rows())?], None, [])
        }),
        Family::Quantizations => parsed.with_quality_quantizations(selection, None, |snapshot| {
            inspect!(context, snapshot, [wire(snapshot.rows())?], None, [])
        }),
        _ => Ok(Err("unexpected quality witness dispatch".into())),
    }
}

fn group_query(parsed: &ParsedCatalogQueries<'_>, context: &Context<'_>) -> QueryResult {
    let selection = context.census.selection();
    match context.family {
        Family::Groups => parsed.with_resource_groups(selection, None, |snapshot| {
            let texts = text_context(
                snapshot.requested_languages().is_empty(),
                snapshot.available_languages().is_empty(),
                snapshot.localization_notes().is_empty(),
                snapshot.public_names(),
                snapshot.game_texts(),
            )?;
            inspect!(
                context,
                snapshot,
                [wire(snapshot.definitions())?, wire(snapshot.members())?],
                None,
                texts
            )
        }),
        Family::Parents => parsed.with_resource_group_parents(selection, None, |snapshot| {
            inspect!(
                context,
                snapshot,
                [wire(snapshot.rows())?],
                Some(snapshot.dependency_coverage()),
                []
            )
        }),
        _ => Ok(Err("unexpected group witness dispatch".into())),
    }
}

fn text_context<P: Serialize + ?Sized, G: Serialize + ?Sized>(
    requested_empty: bool,
    available_empty: bool,
    notes_empty: bool,
    public_names: &P,
    game_texts: &G,
) -> TestResult<[Value; 2]> {
    if !(requested_empty && available_empty && notes_empty) {
        return Err("selective installed SDK witness requires zero locale payloads".into());
    }
    Ok([wire(public_names)?, wire(game_texts)?])
}

fn wire<T: Serialize + ?Sized>(rows: &T) -> TestResult<Value> {
    serde_json::to_value(rows).map_err(|_| "selected SDK row serialization failed".into())
}

fn cancelled_request(
    parsed: &ParsedCatalogQueries<'_>,
    family: Family,
    selection: CatalogRecordSelection,
) -> TestResult {
    let cancel = CancelToken::default();
    cancel.cancel();
    let called = Cell::new(false);
    let result = match family {
        Family::Globals => {
            parsed.with_mining_global_params(selection, Some(&cancel), |_| called.set(true))
        }
        Family::Refining => {
            parsed.with_refining_processes(selection, Some(&cancel), |_| called.set(true))
        }
        Family::Distributions => {
            parsed.with_quality_distributions(selection, Some(&cancel), |_| called.set(true))
        }
        Family::Quantizations => {
            parsed.with_quality_quantizations(selection, Some(&cancel), |_| called.set(true))
        }
        Family::Groups => {
            parsed.with_resource_groups(selection, Some(&cancel), |_| called.set(true))
        }
        Family::Parents => {
            parsed.with_resource_group_parents(selection, Some(&cancel), |_| called.set(true))
        }
    };
    if !matches!(result, Err(CatalogError::Cancelled)) || called.get() {
        return Err(
            "pre-cancelled public SDK request invoked its callback or returned success".into(),
        );
    }
    Ok(())
}

fn after_callback_cancellation(
    parsed: &ParsedCatalogQueries<'_>,
    selection: CatalogRecordSelection,
) -> TestResult {
    let cancel = CancelToken::default();
    let called = Cell::new(false);
    let result = parsed.with_mining_global_params(selection, Some(&cancel), |_| {
        called.set(true);
        cancel.cancel();
    });
    if !called.get() || !matches!(result, Err(CatalogError::Cancelled)) {
        return Err("after-callback cancellation must prevent outer SDK success".into());
    }
    Ok(())
}
