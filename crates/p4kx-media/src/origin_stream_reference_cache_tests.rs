// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::{DecimalU64, FieldOrigin, InputScope, SourceInput, SourceLocation};
use p4kx_extraction::{SourceUse, SourceUseRole, SourceUses};
use serde::Serialize;

use super::Cache;
use crate::origin_contract::{OriginField, OriginRow, OriginScope, OriginStream};
use crate::origin_contributions::{OriginContributions, OriginCut};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum ReferenceRow<'a> {
    Field {
        scope: &'a OriginScope,
        field: &'a str,
        #[serde(flatten)]
        origin: &'a FieldOrigin,
        cuts: &'a [OriginCut],
    },
}

#[test]
fn repeated_reference_array_preserves_the_exact_field_stream_bytes() -> TestResult {
    let owner = SourceUses::new();
    let first = usage(&owner, "escaped \"\\\n\u{0001} é")?;
    let second = usage(&owner, "one")?;
    let third = usage(&owner, "two")?;
    let origin = |uses, complete, cuts: &[OriginCut]| OriginContributions {
        shared: None,
        owner: owner.clone(),
        uses,
        complete,
        cuts: cuts.into(),
    };
    let shared: Arc<[SourceUse]> = [second.clone(), first.clone()].into();
    let first_origin = origin(Arc::clone(&shared), true, &[]);
    let copied = origin(
        [second.clone(), first].into(),
        true,
        &[OriginCut::UseMetadata],
    );
    let incomplete = origin(Arc::clone(&shared), false, &[OriginCut::FieldUses]);
    let different = origin([second, third].into(), true, &[]);
    let independent = OriginContributions::independent(&owner);
    let cases = [
        (&first_origin, false),
        (&copied, true),
        (&independent, false),
        (&first_origin, true),
        (&incomplete, false),
        (&incomplete, true),
        (&different, false),
        (&different, true),
    ];
    let mut cache = Cache::default();
    let scope = OriginScope {
        stream: OriginStream::Sources,
        ordinal: DecimalU64::new(21),
    };
    let field = "/data/name~1with~0escape\n";
    for (contribution, hit) in cases {
        check_field(&mut cache, &scope, field, contribution, hit)?;
    }
    Ok(())
}

fn usage(
    owner: &SourceUses,
    field: &str,
) -> Result<SourceUse, p4kx_extraction::SourceEvidenceError> {
    owner
        .observe(SourceInput::Unknown, InputScope::Payload)?
        .at(
            SourceLocation::Metadata {
                field: field.to_owned(),
            },
            SourceUseRole::Selector,
        )
}

fn check_field(
    cache: &mut Cache,
    scope: &OriginScope,
    field: &str,
    contribution: &OriginContributions,
    hit: bool,
) -> TestResult {
    let expected = serde_json::to_vec(&ReferenceRow::Field {
        scope,
        field,
        origin: &contribution.field_origin()?,
        cuts: &contribution.cuts,
    })?;
    let actual = cache.with_origin(contribution, |origin, encoded| {
        assert_eq!(
            encoded.is_some(),
            hit,
            "cache uses this exact input identity"
        );
        Ok(serde_json::to_vec(&OriginRow::Field {
            scope,
            field,
            origin: OriginField::new(origin, encoded),
            cuts: &contribution.cuts,
        })?)
    })?;
    assert_eq!(
        actual, expected,
        "field origin keeps the canonical inline origin bytes"
    );

    Ok(())
}
