// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::{Instant, UNIX_EPOCH};

use p4kx_export::{
    CatalogOptions, CatalogProgress, CatalogPublicationMode, CatalogSummary, ExportStageStatus,
    Kind, Language,
};
use p4kx_extraction::{Archive, ArchiveWatch};

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
    let (summary, catalog) = census::write_catalog(&archive, &options, || {
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
    drop(summary);
    watch.check().map_err(|_| "Archive changed during write")?;
    if catalog
        .pointer("/source/channel")
        .and_then(serde_json::Value::as_str)
        != Some("LIVE")
    {
        return Err("per-kind baselines require the installed LIVE Channel".into());
    }
    verify_build(&archive, &catalog)?;
    check_texts(&archive, &catalog, true)?;
    watch
        .check()
        .map_err(|_| "Archive changed during source comparison")?;
    Ok(())
}

fn complete_text_scope(summary: &CatalogSummary) -> TestResult {
    for language in [Language::English, Language::German] {
        if !summary.text(language).is_some_and(|text| text.available) {
            return Err(format!("the selected {language} source was not read").into());
        }
    }
    let health = summary.export_health();
    for kind in BASELINES
        .into_iter()
        .map(|baseline| baseline.kind)
        .chain([Kind::ResourceTypes])
    {
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
