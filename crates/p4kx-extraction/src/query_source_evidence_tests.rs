// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_dataforge::query::{QueryByteUse, QuerySource, RecordNameKind};

use super::QuerySourceSite;
use crate::{InputScope, SourceInput, SourceLocation, SourceUseRole, SourceUses};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn shared_query_locations_preserve_original_roles() -> TestResult {
    let cases = [
        (
            QuerySource::Record { index: 17 },
            SourceLocation::DataForge {
                record_index: 17,
                field: "instance".to_owned(),
            },
            SourceUseRole::Selector,
        ),
        (
            QuerySource::ReferenceTarget {
                index: 2,
                kind: RecordNameKind::Path,
            },
            SourceLocation::DataForge {
                record_index: 2,
                field: "fileName".to_owned(),
            },
            SourceUseRole::Selector,
        ),
        (
            QuerySource::Bytes {
                start: 3,
                end: 7,
                kind: QueryByteUse::Value,
            },
            location("dataforgeDecodedBytes", "slot", vec![3, 7]),
            SourceUseRole::Payload,
        ),
        (
            QuerySource::ArrayElement {
                header_start: 11,
                index: 3,
            },
            location("dataforgeArrayElement", "selection", vec![11, 3]),
            SourceUseRole::Selector,
        ),
        (
            QuerySource::ReferenceGap {
                record: 1,
                ordinal: Some(5),
            },
            location("dataforgeReferenceIndex", "unreadableValue", vec![1, 5]),
            SourceUseRole::Selector,
        ),
    ];
    check_locations(cases)
}

fn check_locations(
    cases: impl IntoIterator<Item = (QuerySource, SourceLocation, SourceUseRole)>,
) -> TestResult {
    let uses = SourceUses::with_compact_use_refs();
    let scope = uses.observe(SourceInput::Unknown, InputScope::Payload)?;
    for (observation, expected, role) in cases {
        let site =
            QuerySourceSite::of(observation, SourceUseRole::Payload).ok_or("known observation")?;
        assert_eq!(
            site.location, expected,
            "original values keep the shared wire coordinate"
        );
        assert_eq!(site.role, role, "the original contributor role");
        let bound = site.bind(&scope)?;
        assert_eq!(bound.role(), role, "binding retains the existing role");
        assert_eq!(
            bound.description().location(),
            &expected,
            "binding retains the existing location"
        );
    }
    Ok(())
}

#[test]
fn callers_can_bind_one_failed_observation_without_a_contributor() -> TestResult {
    let uses = SourceUses::with_compact_use_refs();
    let scope = uses.observe(SourceInput::Unknown, InputScope::Payload)?;
    let mut site = QuerySourceSite::of(
        QuerySource::ReferenceGap {
            record: 3,
            ordinal: None,
        },
        SourceUseRole::Payload,
    )
    .ok_or("the reported physical read gap")?;
    site.role = SourceUseRole::Attempted;
    let bound = site.bind(&scope)?;
    assert_eq!(
        bound.role(),
        SourceUseRole::Attempted,
        "the registered link is only an attempt"
    );
    assert_eq!(
        uses.freeze()?.iter_uses().count(),
        1,
        "no spurious contributing location was registered"
    );
    Ok(())
}

fn location(namespace: &str, field: &str, indices: Vec<u64>) -> SourceLocation {
    SourceLocation::Subresource {
        namespace: namespace.to_owned(),
        field: field.to_owned(),
        indices,
    }
}
