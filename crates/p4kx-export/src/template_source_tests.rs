// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::mem::size_of;

use p4kx_extraction::{DecimalU64, SourceUses, SourceValue};

use super::{TemplateSourceValue, TemplateValueMeaning};
use crate::source_value_catalog::{SourceNode, SourcePayload};
use crate::template_source_meaning::ConditionMeaning;

#[test]
fn values_keep_only_a_discriminant_and_pointer_for_meaning() {
    assert!(
        size_of::<TemplateValueMeaning>() <= 2 * size_of::<usize>(),
        "all original value rows must carry fixed-size meaning storage"
    );
}

#[test]
fn original_value_and_nullable_meanings_keep_their_wire_order() -> Result<(), serde_json::Error> {
    let sources = SourceUses::new();
    let mut value = TemplateSourceValue {
        source_ordinal: DecimalU64::new(7),
        original: SourceNode {
            value: sources.value(
                SourceValue::Present(SourcePayload::Boolean { value: false }),
                &[],
                true,
            ),
            original_error: None,
        },
        meaning: TemplateValueMeaning::None,
    };
    let raw = concat!(
        r#"{"sourceOrdinal":"7","value":{"state":"present","value":{"type":"boolean","value":false},"#,
        r#""containsEncryptedSource":false,"sourceUsesRefs":[],"sourceSetComplete":true},"#,
        r#""originalError":null,"condition":"#,
    );
    assert_eq!(
        serde_json::to_string(&value)?,
        format!(r#"{raw}null,"action":null,"randomPhase":null,"trigger":null}}"#),
        "an ordinary raw occurrence retains all four null meaning fields"
    );
    value.meaning = TemplateValueMeaning::Condition(Box::new(ConditionMeaning::OnMissionStart));
    assert_eq!(
        serde_json::to_string(&value)?,
        format!(
            r#"{raw}{{"type":"onMissionStart"}},"action":null,"randomPhase":null,"trigger":null}}"#
        ),
        "the selected meaning stays in the corresponding field"
    );
    Ok(())
}
