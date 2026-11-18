// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io::{self, Write};

use p4kx_extraction::{CancelToken, Guid, SourceUses, SourceValue};

use super::StagedLoadouts;
use crate::CatalogFile;
use crate::catalog_rows::spool::RowSpoolError;
use crate::contract_loadout::{EntityKind, Loadout, LoadoutPort};
use crate::guid_text::CatalogGuid;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn row(identity: u32, kind: EntityKind) -> Loadout {
    let sources = SourceUses::default();
    let name = |value| sources.value(SourceValue::Present(value), &[], false);
    Loadout {
        root_record_index: identity,
        source_record_index: crate::contract_loadout::fixture_record_index(identity),
        port_inventory_complete: true,
        entity_guid: CatalogGuid(Guid::from_stored(u128::from(identity).to_le_bytes())),
        entity_kind: kind,
        issues: Vec::new(),
        ports: vec![LoadoutPort {
            ports: vec![LoadoutPort::named(name("nested".to_owned()))],
            ..LoadoutPort::named(name(format!("port-{identity}")))
        }],
    }
}

#[test]
fn staged_items_and_retained_ships_replay_exact_wire_order() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let rows = vec![
        row(1, EntityKind::Ship),
        row(2, EntityKind::Item),
        row(3, EntityKind::Item),
        row(4, EntityKind::Ship),
        row(5, EntityKind::Item),
        row(6, EntityKind::Ship),
    ];
    let expected = serde_json::to_vec(&rows)?;
    let staged = StagedLoadouts::stage(rows, scratch.path(), None)?;
    assert_eq!(staged.len(), 6, "all canonical roots count");
    assert!(
        staged.stored_bytes() > 0,
        "three Item stages have a persistent compressed frame"
    );
    let ships = staged.ships();
    assert_eq!(
        ships.iter().map(|ship| ship.ordinal).collect::<Vec<_>>(),
        [0, 3, 5],
        "ship joins keep original wire positions"
    );
    assert!(
        ships
            .iter()
            .all(|ship| ship.loadout.port_inventory_complete)
    );
    for _ in 0..2 {
        assert_eq!(
            serde_json::to_vec(&staged)?,
            expected,
            "exact original JSON bytes on replay"
        );
    }
    let ships = vec![row(1, EntityKind::Ship)];
    let expected_ships = serde_json::to_vec(&ships)?;
    let ships_only = StagedLoadouts::stage(ships, scratch.path(), None)?;
    assert_eq!(
        ships_only.stored_bytes(),
        0,
        "ships are never written to the spool"
    );
    assert_eq!(serde_json::to_vec(&ships_only)?, expected_ships);
    assert_eq!(serde_json::to_vec(&StagedLoadouts::default())?, b"[]");
    Ok(())
}

struct CancelAfterWrite<'file> {
    file: &'file mut CatalogFile<'static>,
    cancel: &'file CancelToken,
}

impl Write for CancelAfterWrite<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let length = self.file.write(bytes)?;
        self.cancel.cancel();
        Ok(length)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

#[test]
fn cancelled_item_replay_preserves_previous_catalog_generation() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let output = scratch.path().join("catalog.json");
    let previous = b"previous complete generation";
    fs::write(&output, previous)?;
    let cancel = CancelToken::new();
    let rows = StagedLoadouts::stage(
        vec![row(1, EntityKind::Item)],
        scratch.path(),
        Some(&cancel),
    )?;
    let mut destination = CatalogFile::create(&output)?;
    let attempt = serde_json::to_writer(
        CancelAfterWrite {
            file: &mut destination,
            cancel: &cancel,
        },
        &rows,
    );
    assert!(attempt.is_err(), "item-spool replay observes cancellation");
    assert!(
        matches!(rows.take_failure(), Some(RowSpoolError::Cancelled)),
        "typed cancellation survives Serde"
    );
    destination.discard()?;
    assert_eq!(
        fs::read(&output)?,
        previous,
        "prior generation stays intact"
    );
    drop(rows);
    let contents = fs::read_dir(scratch.path())?.collect::<Result<Vec<_>, _>>()?;
    assert_eq!(
        contents.len(),
        1,
        "the unpublished generation and item frame are gone"
    );
    assert!(
        matches!(
            StagedLoadouts::stage(
                vec![row(2, EntityKind::Item)],
                scratch.path(),
                Some(&cancel)
            ),
            Err(RowSpoolError::Cancelled)
        ),
        "cancelled staging is refused before it opens an item spool"
    );
    Ok(())
}
