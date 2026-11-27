// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::PathBuf;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use p4kx_export::{
    CatalogOptions, CatalogProgress, CatalogPublicationMode, CatalogSummary, ExportHealthStatus,
    ExportStageStatus, Kind, Language,
};
use p4kx_extraction::{Archive, ArchiveWatch, utc_rfc3339};
use serde_json::Value;

use super::{BASELINES, TestResult, check_texts};
use crate::real_archive_shop_inventory_v3::verify_build;
use crate::real_archive_verification::print_line;

#[path = "real_archive_text_census.rs"]
pub(super) mod census;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn installed_live_catalog_texts_match_the_independent_source_oracle() -> TestResult {
    let path = p4kx_testkit::real_archive_path().map_err(|_| "configure the LIVE Archive")?;
    let key = p4kx_testkit::real_archive_key().map_err(|_| "read configured Archive key")?;
    let archive = Archive::open_with_key(path, Some(key)).map_err(|_| "open the LIVE Archive")?;
    let watch = ArchiveWatch::new(&archive);
    let catalog = if let Some(directory) = std::env::var_os("P4KX_CATALOG_MEASUREMENT_DIR") {
        let catalog = census::read_completed_catalog(&PathBuf::from(directory))?;
        verify_archive_stamp(&catalog, archive.file_len(), archive.file_modified())?;
        complete_artifact_scope(&catalog)?;
        print_line("text census read the completed catalog measurement");
        catalog
    } else {
        write_live_catalog(&archive)?
    };
    watch
        .check()
        .map_err(|_| "Archive changed during the catalog read")?;
    if catalog.pointer("/source/channel").and_then(Value::as_str) != Some("LIVE") {
        return Err("per-kind baselines require the installed LIVE Channel".into());
    }
    verify_build(&archive, &catalog)?;
    let comparison = check_texts(&archive, &catalog, true);
    watch
        .check()
        .map_err(|_| "Archive changed during source comparison")?;
    comparison
}

fn write_live_catalog(archive: &Archive) -> census::TestResult<Value> {
    let start = Instant::now();
    let progress = |progress: CatalogProgress| {
        print_line(&format!(
            "text census {:?} {}/{} after {:.1}s",
            progress.phase,
            progress.records_done,
            progress.records_total,
            start.elapsed().as_secs_f64()
        ));
    };
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        progress: Some(&progress),
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    print_line("text census preparing catalog");
    let (summary, catalog) = census::write_catalog(archive, &options, || {
        print_line(&format!(
            "text census writing JSON after {:.1}s",
            start.elapsed().as_secs_f64()
        ));
    })?;
    print_line(&format!(
        "text census decoded {} bytes after {:.1}s",
        summary.size(),
        start.elapsed().as_secs_f64()
    ));
    complete_text_scope(&summary)?;
    Ok(catalog)
}

fn verify_archive_stamp(catalog: &Value, size: u64, modified: Option<SystemTime>) -> TestResult {
    let modified = utc_rfc3339(modified.ok_or("the installed Archive has no modification time")?);
    if catalog
        .pointer("/source/archiveSize")
        .and_then(Value::as_u64)
        != Some(size)
        || catalog
            .pointer("/source/archiveModified")
            .and_then(Value::as_str)
            != Some(modified.as_str())
    {
        return Err("the measured catalog belongs to a different Archive stamp".into());
    }
    Ok(())
}

fn complete_artifact_scope(catalog: &Value) -> TestResult {
    let completeness = catalog
        .get("completeness")
        .and_then(Value::as_object)
        .ok_or("missing catalog selection coverage")?;
    let is_complete = |kind: &Value| kind.get("status").and_then(Value::as_str) == Some("complete");
    if Kind::ALL
        .into_iter()
        .any(|kind| !completeness.get(kind.name()).is_some_and(is_complete))
        || !completeness.values().all(is_complete)
    {
        return Err("the text census requires an unfiltered measured catalog".into());
    }
    for language in [Language::English, Language::German] {
        let state = format!("/texts/languageSources/{}/state", language.code());
        if catalog.pointer(&state).and_then(Value::as_str) != Some("present") {
            return Err(format!("the selected {language} source was not read").into());
        }
    }
    complete_artifact_health(catalog)
}

fn complete_artifact_health(catalog: &Value) -> TestResult {
    let health = catalog
        .get("exportHealth")
        .ok_or("missing catalog export health")?;
    let stage = serde_json::to_value(ExportStageStatus::Complete)?;
    for (field, expected) in [
        (
            "status",
            serde_json::to_value(ExportHealthStatus::Complete)?,
        ),
        ("read", stage.clone()),
        ("projection", stage.clone()),
        (
            "publicationMode",
            serde_json::to_value(CatalogPublicationMode::Normal)?,
        ),
    ] {
        if health.get(field) != Some(&expected) {
            return Err(
                format!("catalog artifact has unexpected export health for {field}").into(),
            );
        }
    }
    let reports = health
        .get("kinds")
        .and_then(Value::as_array)
        .ok_or("kind source accounting")?;
    for kind in required_kinds() {
        let wire_kind = serde_json::to_value(kind)?;
        let mut matches = reports
            .iter()
            .filter(|report| report.get("kind") == Some(&wire_kind));
        let report = matches
            .next()
            .ok_or("missing required text kind source accounting")?;
        if matches.next().is_some()
            || ["sourceInventoryComplete", "fieldInventoryComplete"]
                .into_iter()
                .any(|field| report.get(field).and_then(Value::as_bool) != Some(true))
            || ["read", "projection"]
                .into_iter()
                .any(|field| report.get(field) != Some(&stage))
        {
            return Err(format!(
                "{kind} has incomplete or ambiguous source or projection coverage"
            )
            .into());
        }
    }
    Ok(())
}

fn complete_text_scope(summary: &CatalogSummary) -> TestResult {
    for language in [Language::English, Language::German] {
        if !summary.text(language).is_some_and(|text| text.available) {
            return Err(format!("the selected {language} source was not read").into());
        }
    }
    let health = summary.export_health();
    for kind in required_kinds() {
        let entry = health.kind(kind).ok_or("text kind source accounting")?;
        if !entry.source_inventory_complete
            || !entry.field_inventory_complete
            || entry.read != ExportStageStatus::Complete
            || entry.projection != ExportStageStatus::Complete
        {
            return Err(format!("{kind} has incomplete source or projection coverage").into());
        }
    }
    Ok(())
}

fn required_kinds() -> impl Iterator<Item = Kind> {
    BASELINES.into_iter().map(|baseline| baseline.kind)
}

#[cfg(test)]
mod tests {
    use serde_json::{Map, Value, json};

    use super::{
        Kind, TestResult, UNIX_EPOCH, complete_artifact_scope, required_kinds, verify_archive_stamp,
    };

    #[test]
    fn measured_texts_require_complete_unfiltered_catalog_coverage() -> TestResult {
        let completeness: Map<String, Value> = Kind::ALL
            .into_iter()
            .map(|kind| (kind.name().to_owned(), json!({"status": "complete"})))
            .collect();
        let kinds = required_kinds()
            .map(|kind| {
                json!({
                    "kind": kind,
                    "sourceInventoryComplete": true,
                    "fieldInventoryComplete": true,
                    "read": "complete",
                    "projection": "complete",
                })
            })
            .collect::<Vec<_>>();
        let mut catalog = json!({
            "completeness": completeness,
            "texts": {"languageSources": {"en": {"state": "present"}, "de": {"state": "present"}}},
            "exportHealth": {
                "status": "complete", "read": "complete", "projection": "complete",
                "publicationMode": "normal", "kinds": kinds,
            },
        });
        complete_artifact_scope(&catalog)?;

        let items = Kind::Items.name();
        catalog["completeness"][items] = json!({"status": "partial", "filter": "example"});
        assert!(
            complete_artifact_scope(&catalog).is_err(),
            "read health does not prove selection coverage"
        );
        catalog["completeness"][items] = json!({"status": "complete"});

        catalog["exportHealth"]["kinds"][0]["fieldInventoryComplete"] = json!(false);
        assert!(
            complete_artifact_scope(&catalog).is_err(),
            "the text oracle requires the original field inventory"
        );
        Ok(())
    }

    #[test]
    fn measured_texts_require_the_installed_archive_stamp() -> TestResult {
        let mut catalog =
            json!({"source": {"archiveSize": 4, "archiveModified": "1970-01-01T00:00:00Z"}});
        verify_archive_stamp(&catalog, 4, Some(UNIX_EPOCH))?;
        assert!(
            verify_archive_stamp(&catalog, 5, Some(UNIX_EPOCH)).is_err(),
            "a different Archive size invalidates reuse"
        );
        catalog["source"]["archiveModified"] = json!("1970-01-01T00:00:01Z");
        assert!(
            verify_archive_stamp(&catalog, 4, Some(UNIX_EPOCH)).is_err(),
            "a different Archive modification time invalidates reuse"
        );
        Ok(())
    }
}
