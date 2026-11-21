// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::Entities;
use p4kx_extraction::{Archive, DataForge, SourceUses};

use crate::blueprint_catalog_rows::{self, BlueprintTables};
use crate::build_derived::particle_library;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_output::Written;
use crate::catalog_projections::weapon_reload::{self, WeaponAnimation};
use crate::catalog_read::original::EntryCapture;
use crate::catalog_table::CatalogTable;
use crate::catalog_v3::faction_identity_definition::{
    self, FactionIdentityDefinition, PreparedFactionIdentity,
};
use crate::catalog_v3::flare_library;
use crate::catalog_v3::radar_audio_configuration::{self, RadarAudioConfiguration};
use crate::catalog_v3::subsumption_event;
use crate::client_profile_settings::{self, ClientProfileSettings};
use crate::collected::Collected;
use crate::options::{CatalogRun, CatalogStage};
use crate::ship_store_slugs::{Resolved, StoreSlugs};
use crate::starmap_report::StarmapReport;
use crate::{
    CatalogProvenance, CatalogSummary, CraftingWarning, LoadoutSummary, PaintSummary, ShopSummary,
    StarmapSummary, StoreSlugSummary,
};
/// What a build knows of a catalog besides its size.
pub(super) struct Parts {
    pub(super) export_health: crate::ExportHealth,
    pub(super) provenance: CatalogProvenance,
    pub(super) kinds: Vec<crate::KindSummary>,
    pub(super) texts: Vec<crate::TextSummary>,
    pub(super) text_kind_coverage: Vec<crate::KindTextSummary>,
    pub(super) notes: Vec<p4kx_extraction::LocalizationNote>,
    pub(super) item_classes: crate::ItemClassSummary,
    pub(super) paints: PaintSummary,
    pub(super) loadouts: LoadoutSummary,
    pub(super) starmap: StarmapSummary,
    pub(super) starmap_report: Box<StarmapReport>,
    pub(super) shops: Box<ShopSummary>,
    pub(super) store_slugs: Box<StoreSlugSummary>,
    pub(super) playability: crate::PlayabilitySummary,
    pub(super) variants: crate::VariantSummary,
    pub(super) quantizations: crate::QuantizationSummary,
    pub(super) warnings: Vec<CraftingWarning>,
    pub(super) archive_key: crate::ArchiveKeyUse,
}

impl Parts {
    pub(super) fn summary<W>(self, written: &Written<W>) -> CatalogSummary {
        CatalogSummary {
            export_health: self.export_health,
            provenance: self.provenance,
            staged_row_bytes: written.staged_row_bytes,
            kinds: self.kinds,
            texts: self.texts,
            text_kind_coverage: self.text_kind_coverage,
            notes: self.notes,
            item_classes: self.item_classes,
            paints: self.paints,
            loadouts: self.loadouts,
            starmap: self.starmap,
            starmap_report: self.starmap_report,
            shops: self.shops,
            store_slugs: self.store_slugs,
            playability: self.playability,
            variants: self.variants,
            quantizations: self.quantizations,
            warnings: self.warnings,
            archive_key: self.archive_key,
            size: written.size,
            compressed_size: written.compressed,
        }
    }
}

pub(super) fn ship_store_slugs<'mapping>(
    mapping: &'mapping StoreSlugs,
    ships: &[crate::contract_entities::Ship<'_>],
    filtered: bool,
) -> Resolved<'mapping> {
    mapping.resolve(
        ships
            .iter()
            .map(|ship| (ship.identity.guid.0, ship.identity.class_name)),
        filtered,
    )
}

pub(super) fn blueprint_tables(
    collected: &mut Collected<'_>,
    forge: &DataForge<'_>,
    origins: &CatalogOrigins,
) -> Result<BlueprintTables, crate::CatalogError> {
    blueprint_catalog_rows::tables(
        std::mem::take(&mut collected.blueprints),
        &collected.missing_blueprint_categories,
        &collected.ledger.sources,
        forge,
        origins,
    )
}

pub(super) fn blueprint_pool_tables<'source>(
    collected: &mut Collected<'source>,
    forge: &DataForge<'_>,
) -> crate::catalog_v3::industry_modules::blueprint_pool_tables::BlueprintPoolTables<'source> {
    crate::catalog_v3::industry_modules::blueprint_pool_tables::tables(
        forge,
        std::mem::take(&mut collected.blueprint_pools),
        std::mem::take(&mut collected.blueprint_reward_sources),
    )
}

pub(super) fn faction_identities<'source>(
    forge: &DataForge<'_>,
    rows: Vec<PreparedFactionIdentity<'source>>,
) -> CatalogTable<FactionIdentityDefinition<'source>> {
    faction_identity_definition::table(forge, rows)
}

pub(super) fn capture_weapon_animation<'source>(
    archive: &Archive,
    entities: &Entities<'source>,
    input: &super::original_input::OriginalCatalogInput<'_, 'source>,
    options: &crate::CatalogOptions<'_>,
    collected: &mut Collected<'source>,
) -> Result<WeaponAnimation<'source>, crate::CatalogError> {
    let weapons = std::mem::take(&mut collected.item_definitions.weapons);
    weapon_reload::read(
        archive,
        entities,
        input,
        &weapons,
        options.cancel,
        &mut collected.ledger,
    )
}

pub(super) fn client_configurations(
    archive: &Archive,
    capture: &EntryCapture<'_>,
    collected: &mut Collected<'_>,
    run: CatalogRun<'_, '_>,
) -> Result<(ClientProfileSettings, CatalogTable<RadarAudioConfiguration>), crate::CatalogError> {
    run.begin(CatalogStage::ProjectConfigurations);
    let options = run.options;
    let settings = client_profile_settings::read(
        archive,
        capture,
        options.filter,
        options.cancel,
        &mut collected.ledger,
    )?;
    collected.client_profile_count = settings.decoded_documents();
    let radar_audio = radar_audio_configuration::read(
        archive,
        archive.entries(),
        capture,
        options.filter,
        options.cancel,
        &mut collected.ledger,
    )?;
    collected.radar_audio_count = radar_audio.rows.len();
    Ok((settings, radar_audio))
}

pub(super) fn collect_loose_xml_sources(
    archive: &Archive,
    capture: &EntryCapture<'_>,
    collected: &mut Collected<'_>,
    run: CatalogRun<'_, '_>,
    origins: &SourceUses,
) -> Result<(), crate::CatalogError> {
    let options = run.options;
    run.begin_compacted(
        CatalogStage::ProjectParticleLibraries,
        origins,
        &collected.ledger,
    )?;
    particle_library::collect(archive, capture, options, collected)?;
    run.begin_compacted(
        CatalogStage::ProjectFlareLibraries,
        origins,
        &collected.ledger,
    )?;
    collected.flare_libraries = flare_library::stage(
        archive,
        capture,
        options,
        &mut collected.ledger,
        &std::env::temp_dir(),
    )?;
    run.begin_compacted(
        CatalogStage::ProjectSubsumptionEvents,
        origins,
        &collected.ledger,
    )?;
    collected.subsumption_events = subsumption_event::stage(
        archive,
        capture,
        options,
        &mut collected.ledger,
        &std::env::temp_dir(),
    )?;
    Ok(())
}

pub(super) fn capture_rows(
    collected: &mut Collected<'_>,
    filter: Option<&p4kx_extraction::Filter>,
    recovery_numbers: Vec<crate::catalog_number_capture::CapturedNumber>,
    configuration_numbers: Vec<crate::catalog_number_capture::CapturedNumber>,
) {
    let mut rows = crate::catalog_rows::snapshot(collected, filter);
    rows.recovery_numbers = recovery_numbers;
    rows.configuration_numbers.extend(configuration_numbers);
    if collected.ledger.catalog_rows.replace(rows).is_some() {
        collected.ledger.conflicting_catalog_rows = true;
    }
}

pub(super) fn export_health(
    collected: &mut Collected<'_>,
    kinds: &[crate::KindSummary],
    input: &super::original_input::OriginalCatalogInput<'_, '_>,
    run: CatalogRun<'_, '_>,
) -> Result<crate::ExportHealth, crate::CatalogError> {
    run.begin(CatalogStage::ExportHealth);
    run.check_cancelled()?;
    input
        .origins()
        .uses
        .compact_record_uses(|| run.check_cancelled())?;
    #[cfg(test)]
    run.capture_retention(
        CatalogStage::ExportHealth,
        &input.origins().uses,
        &[&collected.ledger],
    )?;
    let options = run.options;
    crate::export_health_prepare::compile_captured(
        std::mem::take(&mut collected.ledger),
        kinds,
        &collected.failures,
        &collected.warnings,
        options.publication_mode,
        input,
        options.cancel,
    )
}

pub(super) fn freeze_provenance(
    uses: &SourceUses,
    run: CatalogRun<'_, '_>,
) -> Result<CatalogProvenance, crate::CatalogError> {
    run.begin(CatalogStage::Provenance);
    Ok(CatalogProvenance::new(
        uses.finish_with(|| run.check_cancelled())?,
    ))
}
