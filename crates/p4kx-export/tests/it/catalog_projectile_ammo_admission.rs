// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::build_catalog;
use p4kx_extraction::Filter;
use serde_json::{Value, json};

use super::{COMBAT_ROWS, fixture};
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

pub(crate) fn assert_retained_ammo_dependencies(catalog: &Value, guids: [u128; 2]) -> TestResult {
    let [primary, secondary] = proof::rows(catalog, "/definitions/ammoDefinitions/rows")? else {
        return Err("the two proved dependencies are retained".into());
    };
    for (row, guid) in [primary, secondary].into_iter().zip(guids) {
        assert_eq!(
            proof::at(row, "/selection")?,
            "dependency",
            "the Ammo owner is outside the Filter"
        );
        assert_eq!(
            proof::at(row, "/guid/value")?,
            &json!(common::guid(guid)),
            "the admitted dependency is the exact physical target"
        );
    }
    Ok(())
}

#[test]
fn secondary_ammo_does_not_depend_on_admission_by_another_weapon() -> TestResult {
    let filter =
        Filter::new("libs/foundry/records/entities/scitem/ships/fixture_projectile_own_route");
    for (other, selected) in [(false, None), (true, None), (true, Some(&filter))] {
        let database = fixture::cross_owner_ammo(other);
        let (_folder, archive) = common::open(&database, DcbOrigin::Encrypted)?;
        let mut options = common::options();
        options.filter = selected;
        let catalog = serde_json::from_slice(
            build_catalog(&archive, options.with_full_provenance())?.json(),
        )?;
        let ammo = proof::rows(&catalog, "/definitions/ammoDefinitions/rows")?
            .iter()
            .find(|row| {
                row.pointer("/guid/value") == Some(&json!(common::guid(fixture::SECONDARY)))
            })
            .ok_or("the exact excluded Ammo definition is reached by the selected action")?;
        assert_eq!(
            proof::at(ammo, "/selection")?,
            "dependency",
            "the authored debug Record remains outside independent admission"
        );
        let combat = proof::rows(&catalog, COMBAT_ROWS)?
            .iter()
            .find(|row| {
                row.pointer("/ownerGuid/value") == Some(&json!(common::guid(fixture::WEAPON)))
                    && row.pointer("/modeIndex/value") == Some(&json!(1))
            })
            .ok_or("the first weapon's original secondary action")?;
        assert_eq!(
            proof::at(combat, "/ammunitionRecordIndex/value")?,
            proof::at(ammo, "/recordIndex/value")?,
            "the secondary action uses the same physical Ammo Record"
        );
        assert_eq!(
            proof::at(combat, "/model/value/type")?,
            "nominalBullet",
            "the secondary action has its Ammo body without another Item's legacy route"
        );
        proof::closure(&catalog)?;
    }
    Ok(())
}
