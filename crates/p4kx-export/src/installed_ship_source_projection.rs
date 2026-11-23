// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_assembly::ArchiveFiles;
use p4kx_extraction::{Archive, DataForge, DataForgeEntry, Record, RecordTree, SourceInventory};

use crate::Kind;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_projection::{CatalogProjection, CatalogTextRegistry};
use crate::catalog_text_contract::{CanonicalText, GameTextRef, PublicNameRef};
use crate::contract::Ship;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::ExportLedger;
use crate::playability_rules::Evidence;
use crate::record_observation::RecordObservation;
use crate::ship::source_projection::direct;
use crate::texts::{TextSources, Texts};

type TestResult<T> = Result<T, Box<dyn Error>>;

pub(crate) struct Projection<T> {
    pub(crate) fields: T,
    pub(crate) inventory: SourceInventory,
    pub(crate) record_inspections: usize,
    pub(crate) file_operations: usize,
}

pub(crate) fn project<'source, T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    forge: &'source DataForge<'source>,
    record: &Record<'source>,
    finish: impl FnOnce(
        &Ship<'_>,
        &[CanonicalText<'_, PublicNameRef>],
        &[CanonicalText<'_, GameTextRef>],
    ) -> TestResult<T>,
) -> TestResult<Projection<T>> {
    let tree = record
        .tree()
        .map_err(|_| "read selected original Ship body")?;
    let sources =
        TextSources::read_languages(archive, &[]).map_err(|_| "initialize the empty locale set")?;
    let origins =
        CatalogOrigins::new(archive, entry, &sources).map_err(|_| "bind original Ship origins")?;
    let input = OriginalCatalogInput::new(forge, &origins);
    let texts = Texts::parse(&sources).map_err(|_| "initialize empty Ship text index")?;
    let registry = CatalogTextRegistry::default();
    let projection = CatalogProjection::new(&texts, &input, &registry).for_kind(Kind::Ships);
    let admission =
        RecordObservation::capture(&input, record).map_err(|_| "admit original Ship Record")?;
    let (playability, mut ledger) = inspect_inputs(&input, &tree, &admission)?;
    let files = ArchiveFiles::new(archive);
    let projected = direct::read(
        &files,
        &projection,
        &input,
        &admission,
        &tree,
        &mut ledger,
        None,
    )
    .map_err(|_| "scoped producer Ship projection failed")?
    .map_err(|_| "scoped producer Ship resolver failed")?;
    let playability = projected.source.playability_proof.decide(
        &tree,
        projected.source.vehicle,
        &playability,
        &input,
    )?;
    let row = Ship::from_read(projected.source, projected.vehicle, playability)?;
    let locales = projection
        .finish_rows()
        .map_err(|_| "finish scoped producer text registry")?;
    let fields = finish(&row, &locales.public_names, &locales.game_texts)?;
    let inventory = origins
        .uses
        .freeze()
        .map_err(|_| "freeze scoped producer Ship sources")?;
    Ok(Projection {
        fields,
        inventory,
        record_inspections: ledger.field_inspections.len(),
        file_operations: ledger.entries.len(),
    })
}

fn inspect_inputs<'source>(
    input: &OriginalCatalogInput<'_, 'source>,
    tree: &RecordTree<'source>,
    admission: &RecordObservation<'_, 'source>,
) -> TestResult<(Evidence, ExportLedger)> {
    let mut ledger = crate::export_field_walk::inspect_observed(
        Some(input),
        tree,
        admission,
        Some(Kind::Ships),
        ExportSourceRole::Selected,
        None,
    )
    .map_err(|_| "collect original Ship field inspection")?;
    let (playability, observed) = Evidence::read_for_ships([tree], input, None)
        .map_err(|_| "collect scoped producer playability evidence")?;
    ledger.merge(observed);
    Ok((playability, ledger))
}
