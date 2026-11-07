// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_extraction::{Archive, DataForgeEntry, Guid};

use super::schema::Schema;
use super::source::{self, Observations};

pub(crate) struct Tagged {
    pub(crate) schema: Schema,
    pub(crate) observations: Observations,
    pub(crate) guid: Guid,
    pub(crate) rejected: BTreeSet<Guid>,
}

pub(crate) fn read() -> super::Result<Tagged> {
    let directory = tempfile::tempdir()?;
    let archive = Archive::open_with_key(
        super::spawns_fixture::write_archive(directory.path())?,
        Some(p4kx_testkit::test_key()),
    )?;
    let loaded = DataForgeEntry::read(&archive)?;
    let forge = loaded.open()?;
    let source = forge
        .records()
        .find(|record| {
            record
                .record_path()
                .is_ok_and(|path| path.ends_with("/unknown_value"))
        })
        .ok_or("unknown tagged fixture absent")?;
    let tree = source.tree()?;
    let mut options = p4kx_export::CatalogOptions::new(std::time::UNIX_EPOCH);
    options.publication_mode = p4kx_export::CatalogPublicationMode::DiagnosticPartial;
    let built = p4kx_export::build_catalog(&archive, &options)?;
    let schema = Schema::read(&forge, &["MissionProperty".to_owned()], &|| false)?;
    let mut observations = Observations::new();
    source::observe(
        &schema,
        source.guid(),
        tree.root(),
        &mut observations,
        &|| false,
    )?;
    Ok(Tagged {
        schema,
        observations,
        guid: source.guid(),
        rejected: built
            .summary()
            .failures()
            .iter()
            .map(|failure| failure.guid)
            .collect(),
    })
}
