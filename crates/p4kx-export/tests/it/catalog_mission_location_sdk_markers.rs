// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogRecordSelection, with_mission_location_templates,
};

use super::{TestResult, assert_source_uses};
use crate::commodity_v3_fixture::{self, DcbOrigin, guid};
use crate::spawns_fixture::{self, LOCATION_A};

#[test]
fn mission_location_tag_markers_distinguish_all_dataforge_origins() -> TestResult {
    for (source, expected) in [
        (DcbOrigin::Plain, Some(false)),
        (DcbOrigin::Encrypted, Some(true)),
        (DcbOrigin::Unknown, None),
    ] {
        let (_folder, archive) = commodity_v3_fixture::open(&spawns_fixture::database(), source)?;
        with_mission_location_templates(
            &archive,
            CatalogRecordSelection::Guid(guid(LOCATION_A).parse()?),
            CatalogLanguageSelection::Selected(&[]),
            None,
            |snapshot| {
                let [row] = snapshot.rows.as_slice() else {
                    return Err("one original template".into());
                };
                let tag = row
                    .tags()
                    .value()
                    .and_then(|tags| tags.first())
                    .ok_or("one known original tag")?;
                let projected = tag.path()?;
                assert_eq!(
                    projected.origin().contains_encrypted_source(),
                    expected,
                    "the value independently preserves the original DCB origin"
                );
                assert_source_uses(&snapshot, projected.origin().source_uses_refs());
                Ok::<_, Box<dyn std::error::Error>>(())
            },
        )??;
    }
    Ok(())
}

#[path = "mission_location_tag_owner_selection.rs"]
mod owner_selection;
