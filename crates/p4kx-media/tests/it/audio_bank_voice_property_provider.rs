// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::json;

use super::{
    Lists, Node, PROPERTY, Publication, RANGE_BITS, TestResult, export, fixture, row, word,
};

#[test]
fn qualified_voice_values_keep_raw_words_and_precise_source_origins() -> TestResult {
    let source = fixture::bank(Node::Sound, 154, 0, Lists::Complete)?;
    export(&source, true, Publication::Complete, |records, evidence| {
        let unknown = row(records, PROPERTY, &[1, 0, 0, 0])?.1;
        assert!(
            unknown.data.pointer("/payload/interpretation").is_none(),
            "unqualified ID stays raw"
        );
        let known = row(records, PROPERTY, &[1, 0, 0, 1])?;
        assert_eq!(
            known.1.data.pointer("/payload/interpretation/property"),
            Some(&json!("high-pass-filter-percent")),
            "version-qualified ID and unit"
        );
        assert_eq!(
            known.1.data.pointer("/payload/interpretation/scalar/value"),
            Some(&json!(0.0)),
            "finite authored zero"
        );
        evidence.interpretation(known, "property", None)?;
        evidence.interpretation(known, "scalar/value", Some("scalar_bits"))?;
        let ranged = row(records, PROPERTY, &[1, 0, 1, 0])?;
        assert_eq!(
            ranged
                .1
                .data
                .pointer("/payload/interpretation/maximum/value"),
            Some(&serde_json::Value::Null),
            "nonfinite interpretation is null"
        );
        evidence.interpretation(ranged, "maximum/value", Some("maximum_bits"))?;
        word(
            evidence,
            ranged,
            "maximum_bits",
            "nodePropertyMaximumBits",
            RANGE_BITS[1],
        )?;
        Ok(())
    })
}
