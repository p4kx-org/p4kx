// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::source_inventory::PhysicalSourceRef;
use crate::source_use::{InputScope, LocationView, SourceLocation, SourceUseRole};
use crate::source_use_ref_encoding;

use super::RetainedLocation;

#[test]
fn packed_dataforge_locations_keep_their_original_wire_coordinates()
-> Result<(), Box<dyn std::error::Error>> {
    let record_index = u64::from(u32::MAX);
    let source = PhysicalSourceRef::new("synthetic-ü".to_owned());
    for location in [
        SourceLocation::DataForge {
            record_index: u32::MAX,
            field: "attrs.ä:mode[17]\nvalue".to_owned(),
        },
        SourceLocation::DataForge {
            record_index: u32::MAX,
            field: String::new(),
        },
        subresource("dataforgeRecordHeader", vec![record_index], "metadataTag"),
        subresource("dataforgeRecordDeclarations", vec![record_index], "$fields"),
        subresource(
            "dataforgeValue",
            vec![record_index, u64::MAX],
            "$error.guid",
        ),
    ] {
        let id = source_use_ref_encoding::encode(
            &source,
            InputScope::Payload,
            SourceUseRole::Selector,
            LocationView::Original(&location),
        );
        let retained = RetainedLocation::new(location.clone(), Some(&id), None);
        let packed = retained.pack_dataforge().ok_or("packed DataForge slot")?;
        let restored = packed.retain(&id);
        assert_eq!(restored, retained, "the stored coordinate round trips");
        assert_eq!(
            serde_json::to_vec(&restored)?,
            serde_json::to_vec(&location)?,
            "the wire form remains byte identical",
        );
        assert_eq!(
            restored.compact_is_materialized(),
            Some(false),
            "serializing the restored view leaves its owned cache empty",
        );
        assert_eq!(restored.original(), &location, "the full original location");
    }
    Ok(())
}

fn subresource(namespace: &str, indices: Vec<u64>, field: &str) -> SourceLocation {
    SourceLocation::Subresource {
        namespace: namespace.to_owned(),
        indices,
        field: field.to_owned(),
    }
}
