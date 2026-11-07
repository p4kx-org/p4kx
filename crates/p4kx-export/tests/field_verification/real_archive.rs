// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::time::SystemTime;

use p4kx_extraction::{Archive, DataForge, DataForgeEntry};
use serde_json::Value;

use super::accounting;
use super::error::{Error, Kind as ErrorKind};
use super::inventory::Inventory;
use super::published_model::{Build, BuildGate, PUBLICATIONS};

fn exclusions(
    forge: &DataForge<'_>,
    inventory: &Inventory,
) -> super::Result<BTreeMap<p4kx_extraction::Guid, String>> {
    let kinds = super::audit_scope::kinds(inventory)?;
    let records = super::source_scope::records(forge, &kinds, &|| false)?;
    let mut exclusions = BTreeMap::new();
    for scoped in records {
        let record = scoped.record;
        if p4kx_export::is_kind_leftover(scoped.kind, record.record_path()?, record.name()?) {
            exclusions.insert(
                record.guid(),
                "catalog development-leftover selection".to_owned(),
            );
        }
    }
    Ok(exclusions)
}

fn archive() -> super::Result<Archive> {
    Ok(Archive::open_with_key(
        p4kx_testkit::real_archive_path()?,
        Some(p4kx_testkit::real_archive_key()?),
    )?)
}

fn inventory() -> super::Result<Inventory> {
    std::env::var_os("P4KX_FIELD_INVENTORY").map_or_else(
        || Ok(super::baseline::inventory()),
        |path| super::published_io::read_json(std::path::Path::new(&path), 4 << 20, &|| false),
    )
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn field_inventory_observes_declared_populated_and_exported_source_values() -> super::Result<()> {
    let archive = archive()?;
    let loaded = DataForgeEntry::read(&archive)?;
    let forge = loaded.open()?;
    let inventory = inventory()?;
    let kinds = super::audit_scope::kinds(&inventory)?;
    let built =
        p4kx_export::build_catalog(&archive, &super::audit_scope::options(SystemTime::now()))?;
    let health = super::audit_scope::verify(built.summary().export_health(), &kinds);
    if let Err(error) = &health {
        super::report(&format!("mapped production health: {error}"));
    }
    let catalog: Value = serde_json::from_slice(built.json())?;
    let exclusions = exclusions(&forge, &inventory)?;
    super::report(&format!(
        "source selection exclusions: {}",
        exclusions.len()
    ));
    let counts = super::coverage::run(
        &forge,
        &inventory,
        &catalog,
        built
            .summary()
            .failures()
            .iter()
            .map(|failure| failure.guid)
            .collect(),
        exclusions,
        &|| false,
    )?;
    for (field, count) in &counts {
        super::report(&format!(
            "{}.{}{}: declared={} present={} populated={} exported={} unhandled={} unreviewed={} rejected={} source-errors={} missing-targets={} missing-export-GUID={} excluded={} target-changed={} newly-populated={}",
            field.owner,
            field.field,
            if field.array_element { "[]" } else { "" },
            count.declared,
            count.present,
            count.populated,
            count.exported,
            count.unhandled,
            count.review == accounting::Review::Unreviewed,
            count.rejected,
            count.read_errors,
            count.missing_targets,
            count.missing_export_guid,
            count.excluded,
            count.target == accounting::Target::Changed,
            count.population == accounting::Population::NewlyPopulated
        ));
    }
    assert_eq!(
        accounting::regressions(&counts),
        0,
        "every declared/populated field requires a reviewed disposition and retained projection"
    );
    health?;
    Ok(())
}

fn actual_build(archive: &Archive) -> super::Result<Build> {
    let build = archive
        .build()
        .ok_or_else(|| Error::new(ErrorKind::Identity, "source has no established Build"))?;
    let game_version = build.branch().strip_prefix("sc-alpha-").ok_or_else(|| {
        Error::new(
            ErrorKind::Identity,
            "source branch does not establish a public game version",
        )
    })?;
    let channel = archive
        .path()
        .parent()
        .and_then(std::path::Path::file_name)
        .and_then(std::ffi::OsStr::to_str)
        .ok_or_else(|| Error::new(ErrorKind::Identity, "source Channel is unavailable"))?;
    Ok(Build {
        game_version: game_version.to_owned(),
        channel: channel.to_owned(),
        change_number: build.change_number().to_owned(),
    })
}

fn report_comparisons(report: &super::published::Report) -> u64 {
    let mut compared = 0_u64;
    for (field, summary) in &report.fields {
        super::report(&format!(
            "{}.{}: compared={} outcomes={:?}",
            field.owner,
            field.field,
            summary.compared(),
            summary.classes
        ));
        compared += summary.compared();
        assert!(
            summary.clean(),
            "a raw comparison has differences, missing selected values or incompatible/unhandled fields"
        );
    }
    compared
}

fn check_source_records(archive: &Archive, kinds: &[p4kx_export::Kind]) -> super::Result<()> {
    let loaded = DataForgeEntry::read(archive)?;
    let forge = loaded.open()?;
    let records = super::source_scope::records(&forge, kinds, &|| false)?;
    super::report(&format!(
        "mapped physical Record identities checked: {}",
        records.len()
    ));
    Ok(())
}

#[test]
#[ignore = "real Archive and offline scunpacked snapshot: set P4KX_DATA_P4K and P4KX_REF_SCUNPACKED and pass --ignored"]
fn published_scunpacked_raw_fields_match_only_an_established_same_build() -> super::Result<()> {
    let Some(root) = std::env::var_os("P4KX_REF_SCUNPACKED") else {
        super::report(
            "skipped: P4KX_REF_SCUNPACKED is unset; no independent reference comparisons",
        );
        return Ok(());
    };
    let dataset = super::published_io::Dataset::load(std::path::Path::new(&root), &|| false)?;
    let archive = archive()?;
    let actual = actual_build(&archive)?;
    let gate = super::published_model::gate(&dataset.snapshot.provenance, &actual, PUBLICATIONS);
    super::report(&format!(
        "published revision/build evidence gate: {gate:?}; generator={:?}; DCB={:?}; outside-selection files={}",
        dataset.snapshot.provenance.generator_revision,
        dataset.snapshot.provenance.dcb_sha256,
        dataset.outside_selection_files
    ));
    if gate != BuildGate::SamePublishedBuild {
        super::report(
            "skipped numerical assertions: no exact reviewed Channel/version/change match",
        );
        return Ok(());
    }
    let kinds = super::audit_scope::from_names(
        dataset
            .snapshot
            .fields
            .iter()
            .map(|field| field.family.group()),
    )?;
    check_source_records(&archive, &kinds)?;
    let built =
        p4kx_export::build_catalog(&archive, &super::audit_scope::options(SystemTime::now()))?;
    let health = super::audit_scope::verify(built.summary().export_health(), &kinds);
    if let Err(error) = &health {
        super::report(&format!("mapped production health: {error}"));
    }
    let catalog: Value = serde_json::from_slice(built.json())?;
    let report = super::published::verify(&dataset, &catalog, &actual, PUBLICATIONS, &|| false)?;
    let compared = report_comparisons(&report);
    super::report(&format!(
        "GUID joins: matched={} reference-only={} P4KX-only={} excluded={}",
        report.matched_guids,
        report.reference_only_guids,
        report.catalog_only_guids,
        report.excluded_guids
    ));
    assert!(
        compared > 0,
        "at least one same-Build independent raw-field comparison is required"
    );
    assert!(
        report.succeeds(),
        "selected source identities and all raw comparison results must reconcile"
    );
    health?;
    Ok(())
}
