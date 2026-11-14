// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::time::UNIX_EPOCH;

use p4kx_export::{CatalogOptions, CatalogPublicationMode, Filter, Kind, build_catalog_from};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, RecordTree, Value as Original,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use crate::authored_definition_witness::{field, verify_array, verify_link, verify_state};
use crate::catalog_v3_consumer as consumer;
use crate::real_archive_auto_spawns::{original::Locations, provenance::Origins};
use crate::real_archive_canonical_texts::{TextDomain, TextIndex, TextReferenceShape};
use crate::real_archive_npc_spawn_definitions::spawn::verify_single;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn mission_item_definition_matches_original_variants_and_targets() -> TestResult {
    let path = real_archive_path().map_err(|_| "configured Archive path")?;
    let key = real_archive_key().map_err(|_| "configured Archive key")?;
    let archive = Archive::open_with_key(path, Some(key)).map_err(|_| "open Archive")?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read original DataForge")?;
    let forge = entry.open().map_err(|_| "open original DataForge")?;
    let tree = select(&forge, &watch)?;
    let filter = Filter::new(tree.record_path());
    let options = CatalogOptions {
        filter: Some(&filter),
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let built = build_catalog_from(&archive, &entry, &options)
        .map_err(|_| "build the source-selected MissionItem catalog")?;
    let catalog = serde_json::from_slice(built.json())?;
    crate::real_archive_shop_inventory_v3::verify_build(&archive, &catalog)?;
    let rows = consumer::rows(&catalog, Kind::MissionItems)?;
    let row = rows
        .iter()
        .find(|row| {
            row.pointer("/recordIndex/value").and_then(Value::as_u64)
                == Some(u64::from(tree.record_index()))
        })
        .ok_or("selected physical MissionItem Record")?;
    assert!(
        consumer::identity(row, Kind::MissionItems)? == tree.guid().to_string(),
        "the selected MissionItem has the original physical GUID"
    );
    let witness = Witness {
        forge: &forge,
        locations: Locations::read(tree.root()),
        origins: Origins::read(&catalog, entry.origin())?,
        texts: TextIndex::new(&catalog)?,
        owner: tree.record_index(),
    };
    witness.compare(&tree, row)?;
    watch.check().map_err(|_| "Archive changed")?;
    crate::real_archive_verification::print_line(
        "Verified one physical MissionItem with its ordered variants and original field sources",
    );
    Ok(())
}

fn select<'source>(
    forge: &'source DataForge<'source>,
    watch: &ArchiveWatch,
) -> TestResult<RecordTree<'source>> {
    for record in forge.records() {
        watch.poll().map_err(|_| "Archive changed")?;
        if record.type_name()? != "MissionItem" {
            continue;
        }
        let tree = record.tree().map_err(|_| "read original MissionItem")?;
        if matches!(tree.field("entityClass"), Some(Original::Reference(_)))
            && matches!(tree.field("tags"), Some(Original::Array(tags)) if !tags.is_empty())
            && let Some(Original::Struct(variants)) = tree.field("stringVariants")
            && let Some(Original::Array(variants)) = variants.field("variants")
            && variants.iter().any(|variant| {
                matches!(variant, Original::Struct(variant)
                    if matches!(variant.field("string"), Some(Original::Locale(key)) if !key.is_empty()))
            })
        {
            return Ok(tree);
        }
    }
    Err("expected a physical MissionItem with tags, entity and a localized variant".into())
}

struct Witness<'catalog, 'source> {
    forge: &'source DataForge<'source>,
    locations: Locations<'source>,
    origins: Origins<'catalog>,
    texts: TextIndex<'catalog>,
    owner: u32,
}

impl<'source> Witness<'_, 'source> {
    fn compare(&self, tree: &RecordTree<'source>, row: &Value) -> TestResult {
        let weight = tree.field("weighting");
        let cell = field(row, "weighting")?;
        verify_single(weight, cell)?;
        self.occurrence(weight, cell)?;

        let entity = tree.field("entityClass");
        let cell = field(row, "entityClass")?;
        verify_link(self.forge, "EntityClassDefinition", entity, cell)?;
        self.occurrence(entity, cell)?;

        let tags = tree.field("tags");
        let cell = field(row, "tags")?;
        self.occurrence(tags, cell)?;
        verify_array(tags, cell, |source, row| {
            verify_link(self.forge, "Tag", source, row)?;
            self.occurrence(source, row)
        })?;

        let variants = tree.field("stringVariants");
        let cell = field(row, "stringVariants")?;
        self.occurrence(variants, cell)?;
        let Some(Original::Struct(variants)) = variants else {
            return verify_state(variants, cell);
        };
        let source = variants.field("variants");
        let cell = field(consumer::present(cell)?, "variants")?;
        self.occurrence(source, cell)?;
        verify_array(source, cell, |source, row| self.variant(source, row))
    }

    fn occurrence(&self, source: Option<&Original<'source>>, cell: &Value) -> TestResult {
        if let Some(source) = source {
            let part = if matches!(source, Original::Struct(_)) {
                "$type"
            } else {
                "$value"
            };
            self.origins
                .occurrence(cell, self.owner, self.locations.ordinal(source)?, part)?;
        }
        Ok(())
    }

    fn variant(&self, source: Option<&Original<'source>>, cell: &Value) -> TestResult {
        self.occurrence(source, cell)?;
        let Some(Original::Struct(variant)) = source else {
            return verify_state(source, cell);
        };
        let row = consumer::present(cell)?;
        let tag = variant.field("tag");
        let cell = field(row, "tag")?;
        verify_link(self.forge, "Tag", tag, cell)?;
        self.occurrence(tag, cell)?;
        let text = variant.field("string");
        let cell = field(row, "string")?;
        self.occurrence(text, cell)?;
        let Some(Original::Locale(key)) = text else {
            return verify_state(text, cell);
        };
        let target = self
            .texts
            .reference(cell, TextDomain::GameText, TextReferenceShape::Required)?
            .ok_or("original variant locale in protected text table")?;
        assert!(
            target
                .source()
                .pointer("/key/value")
                .and_then(Value::as_str)
                == Some(*key),
            "the original variant Locale key is preserved"
        );
        Ok(())
    }
}
