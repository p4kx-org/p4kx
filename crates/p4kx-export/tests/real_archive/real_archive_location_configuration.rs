// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::error::Error;
use std::time::UNIX_EPOCH;

use p4kx_export::{
    CatalogOptions, CatalogPublicationMode, Kind, build_catalog_from, is_kind_leftover,
};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, DataForgeOrigin, Filter, InputScope,
    RecordTree, SourceEncryption, SourceInput, SourceUses, StructValue, TreeBuilder,
    Value as Original,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use crate::authored_definition_witness::field;
use crate::catalog_v3_consumer as consumer;
use crate::real_archive_auto_spawns::original::Locations;
use crate::real_archive_auto_spawns::provenance::Origins;
use crate::real_archive_item_durability::{self as durability, Trace};

#[path = "location_configuration_fields.rs"]
mod fields;
#[path = "location_module_fields.rs"]
mod modules;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const KIND: Kind = Kind::MissionLocationTemplates;
const SOURCE_TYPE: &str = "MissionLocationTemplate";
const PREFIX: &str = "libs/foundry/records/missiondata/pu_locations/";

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn mission_location_configurations_match_original_selected_owners() -> TestResult {
    let path = real_archive_path().map_err(|_| "configured Archive path")?;
    let key = real_archive_key().map_err(|_| "configured Archive key")?;
    let archive = Archive::open_with_key(path, Some(key)).map_err(|_| "open configured Archive")?;
    let watch = ArchiveWatch::new(&archive);
    watch.check().map_err(|_| "Archive changed")?;
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read original DataForge")?;
    let forge = entry.open().map_err(|_| "open original DataForge")?;
    let scope = std::env::var("P4KX_LOCATION_CONFIGURATION_FILTER")
        .ok()
        .map(|filter| Filter::parse(&filter).map_err(|_| "invalid configuration scope"))
        .transpose()?;
    let selected = Witnesses::read(&forge, &watch, scope.as_ref())?;
    for index in selected.indices()? {
        let record = forge.record(index).ok_or("original location owner")?;
        let tree = TreeBuilder::new()
            .build(&record)
            .map_err(|_| "read location owner")?;
        compare(&archive, &entry, &forge, &tree)?;
        watch.check().map_err(|_| "Archive changed")?;
    }
    Ok(())
}

#[derive(Default)]
struct Witnesses {
    spawn: Option<Witness>,
    limits: Option<Witness>,
    modules: Option<Witness>,
    crew: Option<Witness>,
    disabled: Option<Witness>,
}

struct Witness {
    index: u32,
    count: usize,
}

impl Witnesses {
    fn read(
        forge: &DataForge<'_>,
        watch: &ArchiveWatch,
        scope: Option<&Filter>,
    ) -> TestResult<Self> {
        let mut builder = TreeBuilder::new();
        let mut selected = Self::default();
        for record in forge.records() {
            watch.poll().map_err(|_| "Archive changed")?;
            if record.type_name().map_err(|_| "original Record type")? != SOURCE_TYPE {
                continue;
            }
            let path = record.record_path().map_err(|_| "original Record path")?;
            if !path
                .replace('\\', "/")
                .to_ascii_lowercase()
                .starts_with(PREFIX)
                || path.contains(['*', '?'])
                || scope.is_some_and(|scope| !scope.matches(path))
                || is_kind_leftover(
                    KIND,
                    path,
                    record.name().map_err(|_| "original Record name")?,
                )
            {
                continue;
            }
            let tree = builder
                .build(&record)
                .map_err(|_| "read original location owner")?;
            if let Some(Original::Struct(data)) = tree.field("locationData") {
                selected.observe(record.index(), data);
            }
        }
        Ok(selected)
    }

    fn observe(&mut self, index: u32, data: &StructValue<'_>) {
        let spawns = array_len(data, "autoSpawnSettings");
        let limits = array_len(data, "missionLimits");
        let modules = modules::richness(data.field("missionModules"));
        retain(&mut self.spawn, index, spawns);
        retain(&mut self.limits, index, limits);
        retain(&mut self.modules, index, modules);
        retain(
            &mut self.crew,
            index,
            modules::authored_crew(data.field("missionModules")),
        );
        if matches!(data.field("disabled"), Some(Original::Bool(true))) {
            retain(&mut self.disabled, index, spawns + limits + modules);
        }
    }

    fn indices(&self) -> TestResult<BTreeSet<u32>> {
        let spawn = self
            .spawn
            .as_ref()
            .ok_or("selected scope lacks authored spawn settings")?;
        let limits = self
            .limits
            .as_ref()
            .ok_or("selected scope lacks authored mission limits")?;
        if std::env::var_os("P4KX_EXPECT_DISABLED_LOCATION_CONFIGURATION").is_some()
            && self.disabled.is_none()
        {
            return Err("selected scope lacks a disabled owner with authored configuration".into());
        }
        if std::env::var_os("P4KX_EXPECT_POPULATED_LOCATION_MODULES").is_some()
            && self.modules.is_none()
        {
            return Err("selected scope lacks an owner with authored modules".into());
        }
        Ok([
            Some(spawn),
            Some(limits),
            self.modules.as_ref(),
            self.crew.as_ref(),
            self.disabled.as_ref(),
        ]
        .into_iter()
        .flatten()
        .map(|witness| witness.index)
        .collect())
    }
}

fn retain(slot: &mut Option<Witness>, index: u32, count: usize) {
    if count != 0 && slot.as_ref().is_none_or(|known| known.count < count) {
        *slot = Some(Witness { index, count });
    }
}

fn array_len(data: &StructValue<'_>, name: &str) -> usize {
    match data.field(name) {
        Some(Original::Array(items)) => items.len(),
        _ => 0,
    }
}

fn compare(
    archive: &Archive,
    entry: &DataForgeEntry,
    forge: &DataForge<'_>,
    tree: &RecordTree<'_>,
) -> TestResult {
    let filter = Filter::new(tree.record_path());
    let options = CatalogOptions {
        filter: Some(&filter),
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let built =
        build_catalog_from(archive, entry, &options).map_err(|_| "build selected location")?;
    let catalog: Value = serde_json::from_slice(built.json()).map_err(|_| "read catalog JSON")?;
    crate::real_archive_shop_inventory_v3::verify_build(archive, &catalog)?;
    let rows = consumer::rows(&catalog, KIND)?;
    let guid = tree.guid().to_string();
    let mut owners = rows.iter().filter(|row| {
        row.get("guid").and_then(Value::as_str) == Some(guid.as_str())
            && row.get("recordPath").and_then(Value::as_str) == Some(tree.record_path())
    });
    let row = owners.next().ok_or("selected original location row")?;
    assert!(
        owners.next().is_none(),
        "one row for the selected physical owner"
    );
    let source = tree.field("locationData").ok_or("original location data")?;
    let Original::Struct(data) = source else {
        return Err("selected original location data must be a Struct".into());
    };
    let disabled = match data.field("disabled") {
        Some(Original::Bool(value)) => Some(*value),
        _ => None,
    };
    assert!(
        field(row, "disabled")?.as_bool() == disabled,
        "a disabled owner retains its authored state and configuration"
    );
    check_configuration(
        &catalog,
        entry,
        forge,
        tree,
        source,
        data,
        field(row, "configuration")?,
    )
}

fn check_configuration<'source>(
    catalog: &Value,
    entry: &DataForgeEntry,
    forge: &DataForge<'source>,
    tree: &RecordTree<'source>,
    source: &Original<'source>,
    data: &StructValue<'source>,
    row: &Value,
) -> TestResult {
    let origins = Origins::read(catalog, entry.origin())?;
    let locations = Locations::read(tree.root());
    let uses = durability::indexed(catalog, "/provenance/uses", "id")?;
    let DataForgeOrigin::Archive(original) = entry.origin() else {
        return Err("original DataForge is not an Archive Entry".into());
    };
    let inventory = SourceUses::new();
    let scope = inventory.observe(SourceInput::Archive(original), InputScope::Payload)?;
    let encrypted = match entry.origin().encryption() {
        SourceEncryption::Encrypted => Some(true),
        SourceEncryption::Unencrypted => Some(false),
        _ => None,
    };
    let trace = Trace::of(tree, &uses, &scope, encrypted);
    fields::Check::new(forge, &origins, &locations, tree.record_index(), &trace)
        .configuration(source, data, row)
}
