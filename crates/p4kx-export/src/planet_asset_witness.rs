// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use p4kx_extraction::{Archive, SourceValueState};
use p4kx_testkit::real_archive_path;

use super::super::{planet_climate_asset, planet_geology_asset};
use super::TestResult;
use crate::ExportAccountingError;
use crate::catalog_read::entry_json_object::{self, EntryJsonObject};
use crate::catalog_read::loose_json::LooseJson;
use crate::catalog_read::original::with_entry_capture;
use crate::entry_source::EntrySource;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn installed_planet_assets_retain_the_observed_field_shapes() -> TestResult {
    let archive = Archive::open(real_archive_path()?)?;
    let manifest = archive.build().ok_or("installed Build receipt")?;
    assert_eq!(manifest.change_number(), "13189690", "exact PTU changelist");
    let selector_file =
        std::env::var_os("P4KX_PLANET_ASSET_WITNESS_PATHS").ok_or("private Entry selectors")?;
    let selectors: HashSet<_> = std::fs::read_to_string(selector_file)?
        .lines()
        .map(crate::starmap_index::key)
        .collect();
    assert_eq!(selectors.len(), 103, "PTU climate and geology additions");
    assert!(
        selectors.iter().all(|path| {
            planet_climate_asset::FAMILY.includes(path)
                || planet_geology_asset::FAMILY.includes(path)
        }),
        "exact climate/geology Entry families"
    );
    let mut census = Census::default();
    with_entry_capture(&archive, |capture| {
        for entry in archive
            .entries()
            .iter()
            .filter(|entry| selectors.contains(&crate::starmap_index::key(entry.path())))
        {
            let source = EntrySource::outer(capture, &archive, entry)?;
            let Ok(read) = LooseJson::read(&archive, entry, &source, None)? else {
                return Err(ExportAccountingError::ConflictingSourceDatum.into());
            };
            let Ok(parsed) = read.parse(capture, None)? else {
                return Err(ExportAccountingError::ConflictingSourceDatum.into());
            };
            let object = EntryJsonObject::new(&parsed, parsed.root, capture.source_uses())?
                .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
            census.check(entry.path(), &object)?;
        }
        Ok(())
    })?;
    assert_eq!(
        census,
        Census {
            climate: 35,
            geology: 68,
            sub_assets: 271,
            growth_buckets: 137,
            mineable: 19,
            empty_mineable: 48
        },
        "verified original shape and path counts"
    );
    Ok(())
}

#[derive(Debug, Default, PartialEq, Eq)]
struct Census {
    climate: usize,
    geology: usize,
    sub_assets: u64,
    growth_buckets: u64,
    mineable: usize,
    empty_mineable: usize,
}

impl Census {
    fn check(
        &mut self,
        path: &str,
        object: &EntryJsonObject<'_, '_>,
    ) -> Result<(), crate::CatalogError> {
        assert_eq!(
            object.field("GUID", entry_json_object::guid)?.state(),
            SourceValueState::Present,
            "physical definition has a GUID"
        );
        if planet_climate_asset::FAMILY.includes(path) {
            self.climate += 1;
            let sub_assets = object.field("subAssets", entry_json_object::array_len)?;
            self.sub_assets += sub_assets.value().expect("original sub-asset array").get();
            let buckets = object.field("growthStageBuckets", entry_json_object::array_len)?;
            self.growth_buckets += buckets.value().expect("original growth array").get();
            return Ok(());
        }
        self.geology += 1;
        let config = object.field("Mineable_cfg_path", entry_json_object::text)?;
        let material = object.field("Mineable_Material_path", entry_json_object::text)?;
        assert_eq!(
            config.state(),
            material.state(),
            "paired mineable declaration"
        );
        if let (Some(config), Some(material)) = (config.value(), material.value()) {
            if config.is_empty() {
                assert!(material.is_empty(), "paired empty path");
                self.empty_mineable += 1;
            } else {
                assert!(
                    config
                        .rsplit_once('.')
                        .is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("cgf")),
                    "authored mineable geometry path"
                );
                assert!(
                    material
                        .rsplit_once('.')
                        .is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("mtl")),
                    "authored mineable material path"
                );
                self.mineable += 1;
            }
        }
        Ok(())
    }
}
