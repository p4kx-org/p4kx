// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForge, Language};

use super::{LawFamily, LawSnapshot};
use crate::build::catalog_queries::unlocalized_record_query::{self, plan::Plan};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_projection::{CatalogProjection, CatalogTextRegistry};
use crate::catalog_queries::{CatalogRecordCoverage, CatalogRecordSelection};
use crate::catalog_query_session::{CatalogQuerySource, checkpoint};
use crate::catalog_v3::law_kinds::{self, LawKinds, LawPick};
use crate::record_observation::RecordObservation;
use crate::texts::{TextSources, Texts};
use crate::{CatalogError, ExportAccountingError, Kind};

#[expect(
    clippy::too_many_arguments,
    reason = "parsed source, selected families and callback"
)]
pub(super) fn query<T>(
    archive: &Archive,
    entry: CatalogQuerySource<'_>,
    forge: &DataForge<'_>,
    family: LawFamily,
    selection: CatalogRecordSelection,
    requested_languages: &[Language],
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(LawSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    let plan = Plan::read_with(forge, selection, cancel, None, |record, _, coverage| {
        let Ok(source_type) = record.type_name() else {
            coverage.unreadable_headers.push(record.index());
            return Ok(false);
        };
        Ok(law_kinds::kind_for_type(source_type).is_some_and(|kind| {
            family.includes(kind) && unlocalized_record_query::selected(record, kind, coverage)
        }))
    })?;
    let selected_languages = if needs_locales(&plan) {
        requested_languages
    } else {
        &[]
    };
    checkpoint(cancel)?;
    let sources = TextSources::read_languages(archive, selected_languages)?;
    checkpoint(cancel)?;
    let origins = entry.with_texts(archive, &sources)?;
    let input = OriginalCatalogInput::new(forge, &origins);
    let texts = Texts::parse(&sources)?;
    let registry = CatalogTextRegistry::default();
    let projection = CatalogProjection::new(&texts, &input, &registry);
    let (rows, coverage) = project(plan, &projection, cancel)?;
    let tables = rows.into_tables(forge);
    let text_rows = projection.finish_rows()?;
    let source_inventory = origins.uses.finish_with(|| checkpoint(cancel))?;
    checkpoint(cancel)?;
    Ok(inspect(LawSnapshot {
        build: archive.build().cloned(),
        infractions: family.selected(Kind::Infractions, tables.infractions),
        infraction_sets: family.selected(Kind::InfractionSets, tables.infraction_sets),
        jurisdictions: family.selected(Kind::Jurisdictions, tables.jurisdictions),
        security_network_manifests: family.selected(
            Kind::SecurityNetworkManifests,
            tables.security_network_manifests,
        ),
        public_names: text_rows.public_names,
        game_texts: text_rows.game_texts,
        requested_languages: requested_languages.to_vec(),
        available_languages: selected_languages
            .iter()
            .copied()
            .filter(|language| sources.entry(*language).is_some())
            .collect(),
        localization_notes: texts.notes().to_vec(),
        source_inventory,
        coverage,
    }))
}

fn needs_locales(plan: &Plan<'_>) -> bool {
    plan.owners
        .iter()
        .any(|owner| owner.tree.type_name() != law_kinds::SECURITY_NETWORK_TYPE)
}

fn project<'source>(
    plan: Plan<'source>,
    projection: &CatalogProjection<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<(LawKinds<'source>, CatalogRecordCoverage), CatalogError> {
    let mut coverage = plan.coverage;
    let mut rows = LawKinds::default();
    for owner in plan.owners {
        checkpoint(cancel)?;
        let kind = law_kinds::kind_for_type(owner.tree.type_name())
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let observed = RecordObservation::capture(projection.original(), &owner.record)?;
        let (row, notes) = LawPick::read(
            kind,
            &owner.tree,
            &projection.for_kind(kind),
            &observed,
            cancel,
        )?;
        if notes
            .iter()
            .any(|note| note.description().issue.blocks_projection())
        {
            coverage.incomplete_fields.push(owner.record.index());
        }
        rows.push(row);
    }
    rows.order();
    Ok((rows, coverage))
}
