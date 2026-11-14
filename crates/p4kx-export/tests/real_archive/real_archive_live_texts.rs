// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{BufReader, Seek as _};
use std::time::UNIX_EPOCH;

use p4kx_export::{
    CatalogOptions, CatalogPublicationMode, CatalogSummary, Compression, ExportStageStatus, Kind,
    Language,
};
use p4kx_extraction::{Archive, ArchiveWatch};
use serde_json::Value;

use super::{BASELINES, TestResult, check_texts};
use crate::real_archive_shop_inventory_v3::verify_build;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn installed_live_catalog_texts_match_the_independent_source_oracle() -> TestResult {
    let path = p4kx_testkit::real_archive_path().map_err(|_| "configure the LIVE Archive")?;
    let key = p4kx_testkit::real_archive_key().map_err(|_| "read configured Archive key")?;
    let archive = Archive::open_with_key(path, Some(key)).map_err(|_| "open the LIVE Archive")?;
    let watch = ArchiveWatch::new(&archive);
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let (summary, mut file) =
        p4kx_export::write_catalog(&archive, &options, Compression::None, tempfile::tempfile)
            .map_err(|_| "write current catalog for LIVE text verification")?;
    complete_text_scope(&summary)?;
    drop(summary);
    watch.check().map_err(|_| "Archive changed during write")?;
    file.rewind()?;
    let catalog: Value =
        serde_json::from_reader(BufReader::new(file)).map_err(|_| "read current catalog JSON")?;
    if catalog.pointer("/source/channel").and_then(Value::as_str) != Some("LIVE") {
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
