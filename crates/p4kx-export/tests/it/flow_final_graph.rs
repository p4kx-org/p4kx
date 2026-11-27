// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceInventory, SourceLocation, SourceUseRole};
use serde_json::{Value, json};

use crate::flow_final_fixture;
use crate::flow_final_origin;
use crate::flow_owner_graph::Owner;
use crate::health_origin_fixture::{Input, TestResult};
use crate::loot_v3_graph as raw;

pub(crate) struct Graph<'catalog> {
    pub(crate) configuration: &'catalog Value,
    pub(crate) values: Vec<&'catalog Value>,
    pub(crate) input_ref: &'catalog str,
    pub(crate) owner: Owner<'catalog>,
}

impl<'catalog> Graph<'catalog> {
    pub(crate) fn new(
        catalog: &'catalog Value,
        inventory: &SourceInventory,
        input: Input,
        index: usize,
        count: usize,
    ) -> TestResult<Self> {
        let definition = definition(catalog, inventory, input)?;
        let template = definition.row;
        let input_ref = definition.input_ref;
        let rows = table(catalog, "/configurations/missionFlows")?;
        assert_eq!(
            rows.len(),
            count,
            "each actual matching declaration or one absence has one row"
        );
        let configuration = rows.get(index).ok_or("canonical flow configuration")?;
        assert!(
            configuration.get("values").is_none(),
            "configuration never duplicates owner facts"
        );
        let physical = json!({"type":"contractTemplate","inputRef":input_ref,
            "recordIndex":0,"recordGuid":raw::present(template.get("guid").ok_or("original GUID")?)?});
        assert_eq!(
            configuration.get("owner"),
            Some(&physical),
            "typed physical owner link"
        );
        let links = raw::array(template, "missionFlows")?;
        assert_eq!(
            links.len(),
            count,
            "definition links every retained source configuration"
        );
        assert_eq!(
            links.get(index),
            Some(&json!({"owner":physical,
            "declarationIndex":configuration.get("declarationIndex").ok_or("source declaration position")?})),
            "definition link resolves to this exact physical owner/declaration"
        );
        let owner = Owner::new(template)?;
        let values = owner.members(configuration)?;
        Ok(Self {
            configuration,
            values,
            input_ref,
            owner,
        })
    }

    pub(crate) fn follow(&self, reference: &Value) -> TestResult<&'catalog Value> {
        self.owner.follow(reference)
    }

    pub(crate) fn field(&self, node: &Value, name: &str) -> TestResult<&'catalog Value> {
        let fields = raw::array(raw::payload(node, "struct")?, "fields")?;
        let found: Vec<_> = fields
            .iter()
            .filter(|field| field.pointer("/name/value") == Some(&json!(name)))
            .collect();
        assert_eq!(found.len(), 1, "unique field in this controlled fixture");
        self.follow(
            found
                .first()
                .ok_or("actual source field")?
                .get("value")
                .ok_or("source field local reference")?,
        )
    }
}

fn table<'catalog>(catalog: &'catalog Value, path: &str) -> TestResult<&'catalog [Value]> {
    let table = catalog
        .pointer(path)
        .ok_or_else(|| format!("required canonical table {path}"))?;
    assert_eq!(
        table.get("availability"),
        Some(&json!("available")),
        "the actual declared family is available"
    );
    raw::array(table, "rows")
}

fn header(cell: &Value, inventory: &SourceInventory, input: Input, input_ref: &str) -> TestResult {
    flow_final_origin::closed(
        cell,
        inventory,
        input.marker(),
        true,
        input_ref,
        &[(
            SourceUseRole::Payload,
            SourceLocation::Subresource {
                namespace: "dataforgeRecordHeader".to_owned(),
                indices: vec![0],
                field: "guid".to_owned(),
            },
        )],
    )
}

struct Definition<'catalog> {
    row: &'catalog Value,
    input_ref: &'catalog str,
}

fn definition<'catalog>(
    catalog: &'catalog Value,
    inventory: &SourceInventory,
    input: Input,
) -> TestResult<Definition<'catalog>> {
    let templates = table(catalog, "/definitions/contractTemplates")?;
    assert!(
        catalog.get("contractTemplates").is_none(),
        "no parallel legacy root dataset"
    );
    assert_eq!(templates.len(), 1, "one actual selected physical template");
    let template = templates.first().ok_or("canonical template row")?;
    let guid = template.get("guid").ok_or("actual source header GUID")?;
    assert_eq!(
        raw::present(guid)?,
        &json!(flow_final_fixture::GUID),
        "actual source identity"
    );
    let input_ref = template
        .get("inputRef")
        .and_then(Value::as_str)
        .ok_or("physical source navigation key")?;
    header(guid, inventory, input, input_ref)?;
    assert_eq!(
        raw::present(
            template
                .get("recordIndex")
                .ok_or("physical source header position")?
        )?,
        &json!(0),
        "actual Record index is independently sourced"
    );
    Ok(Definition {
        row: template,
        input_ref,
    })
}
