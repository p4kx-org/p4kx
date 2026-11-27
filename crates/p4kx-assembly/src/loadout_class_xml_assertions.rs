// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::super::test_source::{Log, TestResult};
use crate::{EntityNode, LoadoutError, LoadoutXmlDecoding};

pub(super) fn verify_xml(node: &EntityNode, log: &Log, binary: bool) -> TestResult {
    assert!(
        log.getters.is_empty(),
        "XML class data is not attributed to a Record text getter"
    );
    assert_eq!(
        (log.xml.len(), log.chosen.len()),
        (2, 4),
        "getters are cached but actual chosen attachments are still observed"
    );
    let first = log.xml.first().ok_or("first actual XML getter")?;
    let second = log.xml.get(1).ok_or("second actual XML getter")?;
    assert_ne!(
        first.node_index, second.node_index,
        "equal text remains two original XML elements"
    );
    assert_eq!(
        (first.attribute_index, second.attribute_index),
        (1, 1),
        "both actual decoders retain itemName after portName in each original element"
    );
    assert!(
        first.operation.is_same_operation(&second.operation),
        "both attributes belong to the same genuine current parse operation"
    );
    assert_eq!(
        matches!(first.decoding, LoadoutXmlDecoding::CryXml { .. }),
        binary,
        "actual binary versus text decoder identity"
    );
    let unresolved: Vec<_> = node.unresolved().collect();
    assert_eq!(
        unresolved.len(),
        2,
        "both XML source slots retain their actual cycle failures"
    );
    for (slot, (_, unresolved)) in unresolved.into_iter().enumerate() {
        let LoadoutError::Cycle { guid } = unresolved.error() else {
            return Err("actual XML class cycle".into());
        };
        let original = &log.xml.get(slot).ok_or("actual XML class slot")?.selection;
        assert_eq!(
            guid.class_selection(),
            Some(original),
            "the exact XML getter reaches the chosen target failure"
        );
        assert!(
            log.chosen.iter().any(|chosen| &chosen.selection == original
                && chosen.record_index == 0
                && chosen.guid == guid.guid()),
            "the error GUID is tied to the actual selected Record borrow"
        );
    }
    Ok(())
}
