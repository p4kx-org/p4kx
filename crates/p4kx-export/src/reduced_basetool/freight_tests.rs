// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_extraction::SourceValueState;

use super::super::cargo_grids::{Capacity, GridFacts};
use super::super::cargo_placements::InstalledGrid;
use super::super::cell::Cell;
use super::super::component::ComponentKey;
use super::{CanonicalCargo, Freight, Mount};
use crate::catalog_queries::equipment::CapacityStatus;
use crate::catalog_v3::resource_network_placements::LoadoutPosition;
use crate::guid_text::CatalogGuid;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[test]
fn missing_flag_and_same_guid_at_wrong_physical_owner_do_not_certify_freight() -> TestResult {
    let guid = CatalogGuid("00000000-0000-4000-8000-000000000001".parse()?);
    let cargo = CanonicalCargo {
        scu: cell(Some(4.0)),
        grids_with_issue: cell(Some(0)),
        unresolved_loadout: false,
        mounts: vec![Mount {
            position: cell(Some(LoadoutPosition::from_port_ordinals(12, &[0, 1])?)),
            capacity_scu: cell(Some(4.0)),
            attachment_index: Some(100),
            attachment_guid: Some(guid),
            inventory_index: Some(200),
        }],
    };
    let mount = [0, 1];
    for (record, flag, expected) in [
        (100, None, None),
        (101, Some(false), None),
        (100, Some(false), Some(4.0)),
        (100, Some(true), Some(0.0)),
    ] {
        let grid = GridFacts {
            key: ComponentKey {
                record,
                guid,
                ordinal: 4,
            },
            capacity: Some(Capacity {
                scu: cell(Some(4.0)),
                status: CapacityStatus::Available,
                inventory_record_index: Some(200),
            }),
            mining_only: cell(flag),
        };
        let installed = BTreeMap::from([(
            mount.as_slice(),
            Some(InstalledGrid {
                grid: &grid,
                placement_encrypted: Some(true),
            }),
        )]);
        let reduced = Freight::calculate(12, &cargo, &installed)?;
        assert_eq!(
            reduced.scu, expected,
            "proof for Record {record} and flag {flag:?}"
        );
        assert_eq!(reduced.complete, expected.is_some());
        assert_eq!(reduced.contains_encrypted_source, Some(true));
        assert_eq!(
            cargo.scu.value,
            Some(4.0),
            "the canonical total stays intact"
        );
    }
    Ok(())
}

fn cell<T>(value: Option<T>) -> Cell<T> {
    Cell {
        state: if value.is_some() {
            SourceValueState::Present
        } else {
            SourceValueState::Absent
        },
        value,
        contains_encrypted_source: Some(false),
    }
}
