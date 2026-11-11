// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::broker_flow_fixture::GUID;
use crate::flow_owner_graph::Owner;
use crate::health_origin_fixture::TestResult;
use crate::loot_v3_graph as raw;
use crate::template_flow_depth_fixture::{Branch, Tail};

pub(crate) struct Broker<'catalog> {
    pub(crate) owner: Owner<'catalog>,
    pub(crate) definition: &'catalog Value,
    pub(crate) configuration: &'catalog Value,
}

impl<'catalog> Broker<'catalog> {
    pub(crate) fn new(catalog: &'catalog Value) -> TestResult<Self> {
        let definitions = catalog
            .pointer("/definitions/missionBrokerEntries/rows")
            .and_then(Value::as_array)
            .ok_or("actual canonical physical Broker Flow owners")?;
        assert_eq!(
            definitions.len(),
            1,
            "one actual emitted physical Broker owner"
        );
        let graph = Self::for_guid(catalog, GUID)?;
        assert_eq!(
            raw::array(graph.owner.source, "roots")?.len(),
            1,
            "one actual missionFlow source root"
        );
        assert_eq!(
            graph.owner.source.pointer("/roots/0/sourcePosition"),
            Some(&json!({
                "parentOrdinal":null,"fieldIndex":"0",
            })),
            "the actual original root Field position is retained"
        );
        Ok(graph)
    }

    pub(crate) fn for_guid(catalog: &'catalog Value, guid: &str) -> TestResult<Self> {
        let definition = definition(catalog, guid)?;
        let physical = physical(definition)?;
        let rows = catalog
            .pointer("/configurations/missionFlows/rows")
            .and_then(Value::as_array)
            .ok_or("shared canonical Flow configurations")?;
        let matching: Vec<_> = rows
            .iter()
            .filter(|row| row.get("owner") == Some(&physical))
            .collect();
        assert_eq!(
            matching.len(),
            1,
            "one typed physical family/input/Record/GUID link"
        );
        let configuration = *matching.first().ok_or("actual Broker Flow configuration")?;
        assert!(
            configuration.get("template").is_none() && configuration.get("values").is_none(),
            "the common owner tag replaces the Template-only key and raw facts stay in D"
        );
        let links = raw::array(definition, "missionFlows")?;
        assert_eq!(
            links,
            [json!({"owner":physical,
            "declarationIndex":configuration.get("declarationIndex").ok_or("original declaration index")?})],
            "the definition links the same exact owner and declaration"
        );
        let owner = Owner::new(definition)?;
        Ok(Self {
            owner,
            definition,
            configuration,
        })
    }

    pub(crate) fn condition(&self) -> TestResult<&'catalog Value> {
        let triggers = raw::array(self.configuration, "triggers")?;
        assert_eq!(triggers.len(), 1, "one actual original trigger");
        let trigger = self.owner.follow(
            triggers
                .first()
                .ok_or("actual original trigger reference")?,
        )?;
        self.owner.follow(
            trigger
                .pointer("/trigger/condition")
                .ok_or("actual typed condition selection")?,
        )
    }

    pub(crate) fn chain(&self, branch: Branch, count: usize, tail: Tail) -> TestResult {
        let mut node = self.condition()?;
        for _ in 0..count {
            let meaning = node
                .get("condition")
                .ok_or("actual nullable condition meaning")?;
            assert_eq!(
                meaning.get("type"),
                Some(&json!(branch.wire_type())),
                "each independently proven original operator remains a distinct occurrence"
            );
            let child = match branch {
                Branch::Not => meaning.get("child").ok_or("actual scalar child binding")?,
                Branch::And | Branch::Or => {
                    let children = raw::array(meaning, "children")?;
                    assert_eq!(children.len(), 1, "the complete original array cardinality");
                    children.first().ok_or("actual original sole child")?
                }
            };
            node = self.owner.follow(child)?;
        }
        let meaning = node.get("condition").ok_or("actual leaf meaning")?;
        match tail {
            Tail::Known => assert_eq!(
                meaning,
                &json!({"type":"onMissionStart"}),
                "the actual known leaf"
            ),
            Tail::Unknown => {
                assert_eq!(
                    meaning,
                    &json!({"type":"unsupported"}),
                    "readable unsupported is retained without success"
                );
                assert_eq!(
                    raw::payload(node, "struct")?.pointer("/sourceType/value"),
                    Some(&json!("MissionFlowCondition_Mystery")),
                    "the exact original unknown type remains raw"
                );
            }
            Tail::Missing => assert_eq!(
                meaning.get("child"),
                Some(&Value::Null),
                "genuine null remains null"
            ),
            Tail::Empty => assert!(
                raw::array(meaning, "children")?.is_empty(),
                "genuine empty remains empty"
            ),
        }
        Ok(())
    }

    pub(crate) fn projected(&self, catalog: &Value) -> TestResult {
        let row = projection(catalog, self.definition)?;
        assert_eq!(
            row.get("broker"),
            self.configuration.get("owner"),
            "the successful context selects the same exact physical Broker as its Flow"
        );
        assert!(
            row.get("missionFlow").is_none() && row.get("source").is_none(),
            "the contextual row carries neither a duplicate Flow nor a duplicate raw arena"
        );
        Ok(())
    }
}

pub(crate) fn definition<'catalog>(
    catalog: &'catalog Value,
    guid: &str,
) -> TestResult<&'catalog Value> {
    let definitions = catalog
        .pointer("/definitions/missionBrokerEntries/rows")
        .and_then(Value::as_array)
        .ok_or("actual canonical physical Broker owners")?;
    let matching: Vec<_> = definitions
        .iter()
        .filter(|row| row.pointer("/guid/value") == Some(&json!(guid)))
        .collect();
    assert_eq!(
        matching.len(),
        1,
        "one actual emitted physical Broker owner with this original identity"
    );
    matching
        .first()
        .copied()
        .ok_or_else(|| "actual original Broker definition".into())
}

fn physical(definition: &Value) -> TestResult<Value> {
    Ok(json!({
        "type":"missionBrokerEntry",
        "inputRef":definition.get("inputRef").ok_or("actual input navigation")?,
        "recordIndex":raw::present(definition.get("recordIndex").ok_or("actual original Record position")?)?,
        "recordGuid":raw::present(definition.get("guid").ok_or("actual original GUID")?)?,
    }))
}

pub(crate) fn projections(catalog: &Value) -> TestResult<&[Value]> {
    assert!(
        catalog.get("missionBrokerEntries").is_none(),
        "no parallel legacy Broker root dataset"
    );
    catalog
        .pointer("/configurations/missionBrokerEntries/rows")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| "canonical Broker contextual rows".into())
}

pub(crate) fn projection<'catalog>(
    catalog: &'catalog Value,
    definition: &Value,
) -> TestResult<&'catalog Value> {
    let physical = physical(definition)?;
    let matching: Vec<_> = projections(catalog)?
        .iter()
        .filter(|row| row.get("broker") == Some(&physical))
        .collect();
    assert_eq!(
        matching.len(),
        1,
        "one successful context for this actual family/input/Record/GUID"
    );
    matching
        .first()
        .copied()
        .ok_or_else(|| "actual Broker contextual projection".into())
}

pub(crate) fn token_state(catalog: &Value, guid: &str, expected_id: &Value) -> TestResult {
    let graph = Broker::for_guid(catalog, guid)?;
    let node = graph.condition()?;
    let meaning = node
        .get("condition")
        .ok_or("actual shared condition meaning")?;
    assert_eq!(
        meaning.get("type"),
        Some(&json!("tokenState")),
        "the unchanged condition meaning"
    );
    let objective = meaning
        .get("objective")
        .ok_or("actual target-field binding")?;
    assert_eq!(
        objective.get("interpretation"),
        Some(&json!("id")),
        "the same required objective-id meaning"
    );
    let field = graph.owner.field(
        objective
            .get("field")
            .ok_or("actual target-field navigation")?,
    )?;
    let selected = graph
        .owner
        .follow(field.get("value").ok_or("actual original id cell")?)?;
    assert_eq!(
        raw::payload(selected, "guid")?.get("value"),
        Some(expected_id),
        "the same original objective id is read from its actual selected cell"
    );
    assert_eq!(
        meaning.get("state"),
        Some(&json!({"type":"known","state":"completed"})),
        "the unchanged original Completed state interpretation"
    );
    graph.projected(catalog)
}
