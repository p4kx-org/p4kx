// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::env::{self, VarError};
use std::time::Instant;

use p4kx_export::catalog_queries::{CatalogLanguageSelection, CatalogQuerySession};

use super::equipment_input;
use crate::real_archive_catalog_table_parity::{Origins, TestResult};
use crate::real_archive_standalone_mining_sdk_parity::{MiningDataset, capture_dataset};
use crate::real_archive_verification::print_line;

const DATASET: &str = "P4KX_MINING_SDK_COLD_DATASET";
const PTU_BUILD: &str = "13260598";

#[test]
#[ignore = "installed Archive: configure the expected Build and measure a fresh process"]
fn installed_standalone_mining_sdk_cold_query() -> TestResult {
    let selected = selected_dataset()?;
    let all = MiningDataset::ALL;
    let datasets = selected
        .as_ref()
        .map_or(all.as_slice(), std::slice::from_ref);
    let started = Instant::now();
    equipment_input::with_archive(|archive| {
        let build = archive
            .build()
            .ok_or("the installed Archive lacks a Build")?
            .change_number();
        print_line(&format!(
            "Mining SDK cold phase=archive-ready build={build} elapsedMs={}",
            started.elapsed().as_millis(),
        ));

        let stage = Instant::now();
        let session =
            CatalogQuerySession::open(archive, CatalogLanguageSelection::AllAvailable, None)?;
        print_line(&format!(
            "Mining SDK cold phase=entry-ready elapsedMs={}",
            stage.elapsed().as_millis(),
        ));

        let stage = Instant::now();
        let parsed = session.parsed(None)?;
        print_line(&format!(
            "Mining SDK cold phase=parsed elapsedMs={}",
            stage.elapsed().as_millis(),
        ));

        for &dataset in datasets {
            let stage = Instant::now();
            let mut origins = Origins::default();
            let table = capture_dataset(archive, &parsed, dataset, &mut origins)?;
            let rows = table.rows().len();
            let path = dataset.path();
            if build == PTU_BUILD && rows != ptu_rows(dataset) {
                return Err(
                    format!("{path}: expected {} rows, got {rows}", ptu_rows(dataset)).into(),
                );
            }
            let (uses, sources) = origins.contributing_counts();
            print_line(&format!(
                "Mining SDK cold dataset={path} rows={rows} sourceUses={uses} physicalSources={sources} elapsedMs={}",
                stage.elapsed().as_millis(),
            ));
        }
        Ok(())
    })?;
    print_line(&format!(
        "Mining SDK cold complete datasets={} elapsedMs={}",
        datasets.len(),
        started.elapsed().as_millis(),
    ));
    Ok(())
}

fn selected_dataset() -> TestResult<Option<MiningDataset>> {
    match env::var(DATASET) {
        Ok(path) => Ok(Some(
            MiningDataset::ALL
                .into_iter()
                .find(|dataset| dataset.path() == path)
                .ok_or_else(|| format!("{DATASET} must name a canonical Mining dataset path"))?,
        )),
        Err(VarError::NotPresent) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

const fn ptu_rows(dataset: MiningDataset) -> usize {
    match dataset {
        MiningDataset::Elements => 43,
        MiningDataset::Compositions => 231,
        MiningDataset::Providers => 49,
        MiningDataset::Globals => 8,
        MiningDataset::Refining => 9,
        MiningDataset::Resources => 209,
        MiningDataset::Entities => 247,
    }
}
