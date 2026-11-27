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
use crate::loadout::join::JoinPort;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn row(identity: u32, kind: EntityKind) -> Loadout {
    let sources = SourceUses::default();
    let name = |value| sources.value(SourceValue::Present(value), &[], false);
    Loadout {
        root_record_index: identity,
        source_record_index: crate::contract_loadout::fixture_record_index(identity),
        port_inventory_complete: true,
        entity_guid: crate::loadout_owner_value::LoadoutOwnerValue::fixture(CatalogGuid(
            Guid::from_stored(u128::from(identity).to_le_bytes()),
        )),
        entity_kind: crate::loadout_owner_value::LoadoutOwnerValue::fixture(kind),
        issues: Vec::new(),
        ports: vec![LoadoutPort {
            ports: vec![LoadoutPort::named(name("nested".to_owned()))],
            ..LoadoutPort::named(name(format!("port-{identity}")))
        }],
    }
}

#[test]
fn staged_rows_replay_exact_wire_order_after_ship_joins_are_taken() -> TestResult {
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
    let mut staged = StagedLoadouts::stage(rows, scratch.path(), None)?;
    assert_eq!(staged.len(), 6, "all canonical roots count");
    assert!(
        staged.stored_bytes() > 0,
        "Ship and Item rows share a compressed frame"
    );
    let ships = staged.ships();
    assert_eq!(
        ships
            .iter()
            .map(|ship| ship.view().record_index)
            .collect::<Vec<_>>(),
        [1, 4, 6],
        "only Ship joins retain their physical owner identities in traversal order"
    );
    check_ship_joins(&staged)?;
    assert_eq!(
        serde_json::to_vec(&staged)?,
        expected,
        "exact original wire before release"
    );
    let taken = staged.take_ship_joins();
    assert!(
        staged.ships().is_empty(),
        "the spool no longer retains the transferred join trees"
    );
    assert!(
        taken
            .into_iter()
            .map(|ship| ship.view().record_index)
            .eq([1, 4, 6]),
        "the final join receives the same physical owners in traversal order"
    );
    assert_eq!(
        serde_json::to_vec(&staged)?,
        expected,
        "exact original wire after release"
    );
    Ok(())
}

#[test]
fn staged_ship_only_and_empty_spools_keep_their_wire_shape() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let ships = vec![row(1, EntityKind::Ship)];
    let expected_ships = serde_json::to_vec(&ships)?;
    let ships_only = StagedLoadouts::stage(ships, scratch.path(), None)?;
    assert!(
        ships_only.stored_bytes() > 0,
        "a Ship-only export stages its wire row"
    );
    assert_eq!(
        serde_json::to_vec(&ships_only)?,
        expected_ships,
        "Ship-only replay retains bytes"
    );
    assert_eq!(
        serde_json::to_vec(&StagedLoadouts::default())?,
        b"[]",
        "an empty spool remains an empty array"
    );
    Ok(())
}

fn check_ship_joins(staged: &StagedLoadouts) -> TestResult {
    for ship in staged.ships() {
        let join = ship.view();
        assert!(
            join.port_inventory_complete,
            "the original inventory completeness remains"
        );
        let [port] = join.ports else {
            return Err("one original port in the join tree".into());
        };
        assert_eq!(
            port.children().first().and_then(JoinPort::name_value),
            Some("nested"),
            "the original child occurrence remains nested"
        );
    }
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
fn cancelled_loadout_replay_preserves_previous_catalog_generation() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let output = scratch.path().join("catalog.json");
    let previous = b"previous complete generation";
    fs::write(&output, previous)?;
    let cancel = CancelToken::new();
    let rows = StagedLoadouts::stage(
        vec![row(1, EntityKind::Ship), row(2, EntityKind::Item)],
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
    assert!(
        attempt.is_err(),
        "the Loadout spool replay observes cancellation"
    );
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
        "the unpublished generation and Loadout frame are gone"
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
        "cancelled staging is refused before it opens the Loadout spool"
    );
    Ok(())
}
