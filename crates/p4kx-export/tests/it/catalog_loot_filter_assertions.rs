// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use serde_json::Value;

use crate::loot_v3_graph::{self as graph, Graph};
use crate::starmap_fixture;

pub(crate) fn step<'catalog>(
    graph: &Graph<'catalog>,
    slots: &[Value],
    index: usize,
    name: &str,
    mode: &str,
) -> Result<&'catalog Value, Box<dyn Error>> {
    let instance = graph.follow(slots.get(index).ok_or("ordered original instance")?)?;
    let fields = graph::structure(instance, "PoolFilterInstance")?;
    assert_eq!(
        fields.len(),
        3,
        "all source label/mode/filter fields remain"
    );
    assert_eq!(
        graph::public_text(graph.field(fields, "name")?)?,
        name,
        "exact original designer label"
    );
    assert_eq!(
        graph::public_text(graph.field(fields, "mode")?)?,
        mode,
        "exact original combination mode"
    );
    graph.field(fields, "filter")
}

pub(crate) fn tags(
    graph: &Graph<'_>,
    term: &Value,
    positive: &[u128],
    negative: &[u128],
) -> Result<(), Box<dyn Error>> {
    let fields = graph::structure(term, "TagsDNFTerm")?;
    for (name, expected) in [("positiveTags", positive), ("negativeTags", negative)] {
        let elements = graph::elements(graph.field(fields, name)?)?;
        assert_eq!(
            elements.len(),
            expected.len(),
            "exact original {name} count, including empty"
        );
        for (reference, guid) in elements.iter().zip(expected) {
            assert_eq!(
                graph::reference_guid(graph.follow(reference)?)?,
                starmap_fixture::guid_text(*guid),
                "exact original {name} target"
            );
        }
    }
    Ok(())
}
