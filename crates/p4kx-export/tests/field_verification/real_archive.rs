// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::path::Path;
use std::time::SystemTime;

use p4kx_channels::PublicPatches;
use p4kx_extraction::{Archive, ArchiveWatch, DataForge, DataForgeEntry, Filter};
use serde_json::Value;

use super::accounting;
use super::error::{Error, Kind as ErrorKind};
use super::inventory::{BuildIdentity, Inventory};
use super::published_io::Dataset;
use super::published_model::{Build, BuildGate, PUBLICATIONS};

struct ReviewedPatch {
    channel: &'static str,
    change_number: &'static str,
    version: &'static str,
}

const RSI_PATCHES: &[ReviewedPatch] = &[ReviewedPatch {
    channel: "LIVE",
    change_number: "13165946",
    version: "4.10.3",
}];

fn exclusions(
    forge: &DataForge<'_>,
    inventory: &Inventory,
    filter: Option<&Filter>,
) -> super::Result<BTreeMap<p4kx_extraction::Guid, String>> {
    let kinds = super::audit_scope::kinds(inventory)?;
    let records = super::source_scope::records(forge, &kinds, filter, &|| false)?;
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
        |path| super::published_io::read_json(Path::new(&path), 4 << 20, &|| false),
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
    let watch = ArchiveWatch::new(&archive);
    let filter = record_filter()?;
    let scope = filter.as_ref();
    if scope.is_some() {
        let selected = super::source_scope::records(&forge, &kinds, scope, &|| false)?;
        assert!(
            !selected.is_empty(),
            "the field-audit filter must select a source Record"
        );
        super::report(&format!(
            "explicit Record filter: {} source owners",
            selected.len()
        ));
    }
    let options = p4kx_export::CatalogOptions {
        filter: scope,
        ..super::audit_scope::options(SystemTime::now())
    };
    let built = p4kx_export::build_catalog_from(&archive, &loaded, &options)?;
    let health = super::audit_scope::verify(built.summary().export_health(), &kinds);
    if let Err(error) = &health {
        super::report(&format!("mapped production health: {error}"));
    }
    let catalog: Value = serde_json::from_slice(built.json())?;
    let exclusions = exclusions(&forge, &inventory, scope)?;
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
            .map(|failure| failure.guid().parse())
            .collect::<Result<_, _>>()?,
        exclusions,
        super::coverage::Scope {
            filter: scope,
            build: BuildIdentity::of_archive(&archive).as_ref(),
        },
        &|| false,
    )?;
    report_counts(&counts);
    watch.check()?;
    assert_eq!(
        accounting::regressions(&counts),
        0,
        "every declared/populated field requires a reviewed disposition and retained projection"
    );
    health?;
    Ok(())
}

fn record_filter() -> super::Result<Option<Filter>> {
    match std::env::var("P4KX_FIELD_RECORD_FILTER") {
        Ok(value) if !value.trim().is_empty() => Ok(Some(Filter::parse(&value)?)),
        Ok(_) => Err(Error::new(ErrorKind::Input, "empty field-audit Record filter").into()),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn report_counts(counts: &BTreeMap<super::inventory::FieldKey, accounting::Counts>) {
    for (field, count) in counts {
        super::report(&format!(
            "{}.{}{}: declaration={:?} present={} populated={} exported={} unhandled={} unreviewed={} rejected={} source-errors={} missing-targets={} missing-export-GUID={} excluded={} target-changed={} newly-populated={}",
            field.owner,
            field.field,
            if field.array_element { "[]" } else { "" },
            count.declaration,
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
}

fn actual_build(archive: &Archive, patches: &PublicPatches) -> super::Result<Option<Build>> {
    let build = archive
        .build()
        .ok_or_else(|| Error::new(ErrorKind::Identity, "source has no established Build"))?;
    let channel = archive
        .path()
        .parent()
        .and_then(Path::file_name)
        .and_then(std::ffi::OsStr::to_str)
        .ok_or_else(|| Error::new(ErrorKind::Identity, "source Channel is unavailable"))?;
    let launcher = build.public_patch().or_else(|| patches.of(build));
    let reviewed = RSI_PATCHES
        .iter()
        .find(|patch| patch.channel == channel && patch.change_number == build.change_number())
        .map(|patch| patch.version);
    if let (Some(launcher), Some(reviewed)) = (launcher, reviewed)
        && launcher != reviewed
    {
        return Err(Error::new(
            ErrorKind::Identity,
            "launcher and reviewed RSI public patches disagree",
        )
        .into());
    }
    let Some(game_version) = launcher.or(reviewed) else {
        return Ok(None);
    };
    Ok(Some(Build {
        game_version: game_version.to_owned(),
        channel: channel.to_owned(),
        change_number: build.change_number().to_owned(),
    }))
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

fn check_source_records(
    forge: &DataForge<'_>,
    kinds: &[p4kx_export::Kind],
    filter: Option<&Filter>,
) -> super::Result<()> {
    let records = super::source_scope::records(forge, kinds, filter, &|| false)?;
    if filter.is_some() && records.is_empty() {
        return Err(Error::new(
            ErrorKind::Input,
            "the Record filter selected no source owners",
        )
        .into());
    }
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
    let dataset = Dataset::load(Path::new(&root), &|| false)?;
    let archive = archive()?;
    let Some(actual) = actual_build(&archive, &PublicPatches::standard())? else {
        super::report(
            "skipped numerical assertions: no proven public patch for the source Channel and change number",
        );
        return Ok(());
    };
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
    compare_snapshot(&archive, &dataset, &actual)
}

fn compare_snapshot(archive: &Archive, dataset: &Dataset, actual: &Build) -> super::Result<()> {
    let kinds = super::audit_scope::from_names(
        dataset
            .snapshot
            .fields
            .iter()
            .map(|field| field.family.group()),
    )?;
    let watch = ArchiveWatch::new(archive);
    let loaded = DataForgeEntry::read(archive)?;
    let forge = loaded.open()?;
    let filter = record_filter()?;
    check_source_records(&forge, &kinds, filter.as_ref())?;
    let options = p4kx_export::CatalogOptions {
        filter: filter.as_ref(),
        ..super::audit_scope::options(SystemTime::now())
    };
    let built = p4kx_export::build_catalog_from(archive, &loaded, &options)?;
    let health = super::audit_scope::verify(built.summary().export_health(), &kinds);
    if let Err(error) = &health {
        super::report(&format!("mapped production health: {error}"));
    }
    let catalog: Value = serde_json::from_slice(built.json())?;
    let report = super::published::verify(dataset, &catalog, actual, PUBLICATIONS, &|| false)?;
    watch.check()?;
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

#[test]
fn public_game_version_is_bound_by_change_even_when_the_internal_branch_differs()
-> super::Result<()> {
    let temp = tempfile::tempdir()?;
    let archive = build_fixture(&temp.path().join("LIVE"), "123")?;
    std::fs::write(temp.path().join("log.log"), b"(SC LIVE 4.10.3-LIVE.124)\n")?;
    assert_eq!(
        actual_build(&archive, &PublicPatches::read_from(temp.path()))?,
        None,
        "a log entry from another change cannot establish the public patch",
    );
    std::fs::write(temp.path().join("log.log"), b"(SC LIVE 4.10.3-LIVE.123)\n")?;
    assert_eq!(
        actual_build(&archive, &PublicPatches::read_from(temp.path()))?,
        Some(Build {
            game_version: "4.10.3".to_owned(),
            channel: "LIVE".to_owned(),
            change_number: "123".to_owned(),
        }),
        "the public label belongs to the selected physical change",
    );
    Ok(())
}

#[test]
fn a_reviewed_rsi_patch_is_bound_to_the_exact_channel_and_change() -> super::Result<()> {
    let temp = tempfile::tempdir()?;
    let live = build_fixture(&temp.path().join("LIVE"), "13165946")?;
    let ptu = build_fixture(&temp.path().join("PTU"), "13165946")?;
    let empty = PublicPatches::default();
    assert_eq!(
        actual_build(&live, &empty)?
            .map(|build| build.game_version)
            .as_deref(),
        Some("4.10.3"),
        "the reviewed release supplies the public patch for its LIVE change",
    );
    assert_eq!(
        actual_build(&ptu, &empty)?,
        None,
        "the same change in another Channel is not a match"
    );
    std::fs::write(
        temp.path().join("log.log"),
        b"(SC LIVE 4.10.2-LIVE.13165946)\n",
    )?;
    assert!(
        actual_build(&live, &PublicPatches::read_from(temp.path())).is_err(),
        "conflicting version evidence is refused",
    );
    Ok(())
}

pub(super) fn build_fixture(channel: &Path, change: &str) -> super::Result<Archive> {
    use p4kx_testkit::{ArchiveBuilder, TestEntry, write_build_manifest};

    std::fs::create_dir(channel)?;
    let path = channel.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/sample", b"fixture"))
        .write_to(&path)?;
    write_build_manifest(channel, "sc-alpha-4.10.0", "4.10.200.1", change, "fixture")?;
    Ok(Archive::open(path)?)
}
