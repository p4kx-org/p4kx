// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, TreeBuilder};
use serde_json::Value;

use super::reference_manifest::Location;
use super::starbreaker_json;
use super::tally::Compared;
use super::value_tree::{Node, RecordDoc};
use super::value_tree_compare::{compare_counting, compare_embedded_counting};

fn record_doc<'f>(
    forge: &'f DataForge<'f>,
    builder: &mut TreeBuilder<'f>,
    guid: &str,
) -> Option<RecordDoc> {
    let found = forge.record_by_guid(guid.parse().ok()?)?;
    let tree = builder.build(&found).ok()?;
    Some(RecordDoc::from_tree(&tree))
}

pub(crate) fn compare<'f>(
    forge: &'f DataForge<'f>,
    builder: &mut TreeBuilder<'f>,
    guid: &str,
    location: &Location,
    document: &Value,
) -> (Compared, u64) {
    let selected = location
        .subresource
        .as_ref()
        .map_or(Some(document), |pointer| document.pointer(pointer));
    let Some(selected) = selected else {
        return (
            Compared::Different("declared JSON subresource is absent".to_owned()),
            0,
        );
    };
    let Some(ours) = record_doc(forge, builder, guid) else {
        return (
            Compared::Different("selected source Record cannot be built".to_owned()),
            0,
        );
    };
    if selected.get(starbreaker_json::RECORD_VALUE_KEY).is_some() {
        return match starbreaker_json::record_value(selected) {
            Ok(theirs) => {
                compare_counting(&ours, &theirs, &mut |guid| record_doc(forge, builder, guid))
            }
            Err(error) => (Compared::Different(error), 0),
        };
    }
    match starbreaker_json::node(selected) {
        Node::Reference {
            guid: other,
            embedded: Some(content),
            ..
        } if other == guid => {
            compare_embedded_counting(guid, &content, &mut |guid| record_doc(forge, builder, guid))
        }
        _ => (
            Compared::Different(
                "location does not contain the selected Record identity/content".to_owned(),
            ),
            0,
        ),
    }
}
