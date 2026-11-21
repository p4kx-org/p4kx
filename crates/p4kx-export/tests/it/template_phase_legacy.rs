// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, DataForgeEntry, Value as Raw};
use serde_json::{Value, json};

use crate::commodity_v3_fixture;
use crate::commodity_v3_fixture::TestResult;
use crate::spawns_fixture;

pub(crate) struct Phase {
    id: String,
    parent: Option<String>,
}

pub(crate) fn source(archive: &Archive) -> TestResult<Vec<Phase>> {
    let input = DataForgeEntry::read(archive)?;
    let forge = input.open()?;
    let identity = commodity_v3_fixture::guid(spawns_fixture::DEEP_PHASES);
    let record = forge
        .records()
        .find(|record| record.guid().to_string() == identity)
        .ok_or("actual legacy deep-phase template")?;
    assert_eq!(
        record.type_name()?,
        "ContractTemplate",
        "actual original family"
    );
    let tree = record.tree()?;
    assert!(tree.is_from(&record), "paired original legacy tree");
    assert_eq!(
        tree.error_count(),
        0,
        "all original legacy phases are readable"
    );
    let Some(Raw::Array(roots)) = tree.field("objectiveTokens") else {
        return Err("actual original objective array".into());
    };
    assert_eq!(roots.len(), 1, "one actual original chain");
    let phases = chain(roots.first())?;
    assert_eq!(
        phases.len(),
        35,
        "the existing original fixture retains all 35 phases"
    );
    Ok(phases)
}

fn chain(mut next: Option<&Raw<'_>>) -> TestResult<Vec<Phase>> {
    let mut phases = Vec::new();
    let mut parent = None;
    while let Some(value) = next {
        let Raw::Struct(node) = value else {
            return Err("readable original phase".into());
        };
        assert_eq!(
            node.type_name(),
            "ObjectiveToken",
            "actual original node type"
        );
        let expected = commodity_v3_fixture::guid(
            spawns_fixture::DEEP_PHASES
                .checked_add(u128::try_from(phases.len())?)
                .ok_or("synthetic phase ID")?,
        );
        assert_eq!(
            node.field("id"),
            Some(&Raw::Guid(expected.parse()?)),
            "actual original ordered ID"
        );
        phases.push(Phase {
            id: expected.clone(),
            parent,
        });
        parent = Some(expected);
        let Some(Raw::Array(children)) = node.field("childMissionPhases") else {
            return Err("actual original phase children".into());
        };
        assert!(children.len() <= 1, "the original fixture is a chain");
        next = children.first();
    }
    Ok(phases)
}

pub(crate) fn output(catalog: &Value, original: &[Phase]) -> TestResult {
    let identity = commodity_v3_fixture::guid(spawns_fixture::DEEP_PHASES);
    let graph = crate::template_canonical_graph::Template::new(catalog, &identity)?;
    let phases = crate::loot_v3_graph::array(graph.projection, "objectives")?;
    assert_eq!(
        phases.len(),
        original.len(),
        "no materialized phase disappears"
    );
    for (actual, expected) in phases.iter().zip(original) {
        let id = graph.bound(actual.get("id").ok_or("actual original id field")?)?;
        assert_eq!(
            crate::template_fixture_fields::scalar(id, "guid")?,
            &json!(expected.id),
            "source preorder ID"
        );
        let parent = match actual.get("parent") {
            Some(Value::Null) => None,
            Some(reference) => {
                let parent = graph.owner.follow(reference)?;
                Some(
                    crate::template_fixture_fields::scalar(graph.field(parent, "id")?, "guid")?
                        .as_str()
                        .ok_or("actual original parent id")?,
                )
            }
            None => return Err("required nullable actual parent link".into()),
        };
        assert_eq!(
            parent,
            expected.parent.as_deref(),
            "actual original immediate parent"
        );
    }
    Ok(())
}
