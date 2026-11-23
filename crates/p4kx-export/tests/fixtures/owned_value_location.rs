// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::{DataForge, SourceLocation, Step, TreeBuilder};

pub(crate) fn value_location(
    forge: &DataForge<'_>,
    record_index: u32,
    matches_path: impl Fn(&[Step<'_>]) -> bool,
) -> Result<SourceLocation, Box<dyn Error>> {
    let record = forge
        .record(record_index)
        .ok_or("missing original fixture Record")?;
    let tree = TreeBuilder::new().build(&record)?;
    let mut locations = Vec::new();
    tree.root().visit_owned_values(|visit| {
        if matches_path(visit.steps()) {
            locations.push(visit.ordinal());
        }
    });
    let [ordinal] = locations.as_slice() else {
        return Err("the original fixture field must occur exactly once".into());
    };
    Ok(SourceLocation::Subresource {
        namespace: "dataforgeValue".to_owned(),
        indices: vec![u64::from(record_index), u64::try_from(*ordinal)?],
        field: "$value".to_owned(),
    })
}
