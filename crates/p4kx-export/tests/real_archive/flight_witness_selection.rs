// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{Archive, DataForge, DataForgeEntry, Filter, RecordTree};
use serde_json::Value;

use super::Result;

pub(super) struct SelectorFile {
    pub(super) spaceship: String,
    pub(super) controller: String,
    pub(super) ground: String,
    pub(super) main: String,
    pub(super) main_role: String,
    pub(super) maneuver: String,
    pub(super) maneuver_role: String,
}

impl SelectorFile {
    pub(super) fn read() -> Result<Self> {
        let file = std::env::var_os("P4KX_FLIGHT_CONFIGURATION_WITNESS")
            .ok_or("private Flight Configuration selectors")?;
        let selectors = std::fs::read_to_string(file).map_err(|_| "read private selectors")?;
        let mut selectors = selectors.lines().map(str::trim);
        let mut next = || {
            selectors
                .next()
                .filter(|line| !line.is_empty())
                .map(str::to_owned)
        };
        let fixture = Self {
            spaceship: next().ok_or("exact spacecraft Record path")?,
            controller: next().ok_or("exact attached controller Record path")?,
            ground: next().ok_or("exact ground vehicle Record path")?,
            main: next().ok_or("exact main thruster Record path")?,
            main_role: next().ok_or("authored main thruster role label")?,
            maneuver: next().ok_or("exact maneuvering thruster Record path")?,
            maneuver_role: next().ok_or("authored maneuvering thruster role label")?,
        };
        if selectors.next().is_some() || fixture.main_role == fixture.maneuver_role {
            return Err("seven lines and two distinct original role labels are required".into());
        }
        Ok(fixture)
    }
}

pub(super) struct Selection<'source> {
    pub(super) tree: RecordTree<'source>,
    filter: Filter,
}

impl<'source> Selection<'source> {
    pub(super) fn read(forge: &'source DataForge<'source>, path: &str) -> Result<Self> {
        if path.contains(['*', '?']) {
            return Err("Archive witness requires a literal Record path".into());
        }
        let mut matches = forge.records_at_path(path);
        let record = matches.next().ok_or("selected original Record")?;
        if matches.next().is_some() || record.type_name().ok() != Some("EntityClassDefinition") {
            return Err("selector must resolve one physical Entity Record".into());
        }
        let filter = Filter::parse(
            record
                .record_path()
                .map_err(|_| "read original Record path")?,
        )
        .map_err(|_| "create narrow Record filter")?;
        let tree = record
            .tree()
            .map_err(|_| "read exact original Entity Record")?;
        Ok(Self { tree, filter })
    }

    pub(super) fn build(&self, archive: &Archive, entry: &DataForgeEntry) -> Result<Value> {
        let options = CatalogOptions {
            filter: Some(&self.filter),
            publication_mode: CatalogPublicationMode::DiagnosticPartial,
            ..CatalogOptions::new(UNIX_EPOCH)
        };
        let catalog = build_catalog_from(archive, entry, &options)
            .map_err(|_| "build selected diagnostic Catalog")?;
        serde_json::from_slice(catalog.json()).map_err(|_| "read selected Catalog JSON".into())
    }
}
