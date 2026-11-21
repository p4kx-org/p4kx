// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::PaintTagIndex;
use p4kx_extraction::TagTree;

use super::{PaintOwner, ShipPaintMatches};
use crate::guid_text::CatalogGuid;
use crate::paint_port::{PaintPort, VehicleFacts, VehiclePorts};

#[test]
fn shared_guid_and_equal_ports_preserve_physical_compatibility()
-> Result<(), Box<dyn std::error::Error>> {
    let guid = CatalogGuid("00000000-0000-0000-0000-000000000001".parse()?);
    let group = ShipPaintMatches::read(&[
        vehicle(11, guid, "alpha"),
        vehicle(12, guid, "beta"),
        vehicle(13, guid, "alpha"),
    ])?;
    let tree = TagTree::default();
    let tags = PaintTagIndex::new(&tree);
    let mut alpha = group.fit(&["ALPHA"], &[], &tags).collect::<Vec<_>>();
    alpha.sort_unstable_by_key(|owner| owner.record_index);
    assert_eq!(
        alpha,
        [
            PaintOwner {
                record_index: 11,
                guid
            },
            PaintOwner {
                record_index: 13,
                guid
            },
        ],
        "both physical ships sharing an admitting group retain their owners"
    );
    let beta = group.fit(&["beta"], &[], &tags).collect::<Vec<_>>();
    assert_eq!(
        beta,
        [PaintOwner {
            record_index: 12,
            guid
        }],
        "another ship with that GUID has its own compatibility decision"
    );
    Ok(())
}

fn vehicle(record_index: u32, guid: CatalogGuid, required: &str) -> VehicleFacts {
    VehicleFacts {
        source_record_index: Some(record_index),
        ship: guid,
        tags: Vec::new(),
        paint_ports: vec![PaintPort::new(None, Some(required))],
        implementation: VehiclePorts::default(),
        variant_key: None,
    }
}
