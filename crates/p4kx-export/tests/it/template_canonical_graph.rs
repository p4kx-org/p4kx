// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::flow_owner_graph::Owner;
use crate::health_origin_fixture::TestResult;
use crate::loot_v3_graph as raw;

/// Read-only structural navigation in the actual canonical D/C producer document.
pub(crate) struct Template<'catalog> {
    pub(crate) owner: Owner<'catalog>,
    pub(crate) projection: &'catalog Value,
    pub(crate) definition: &'catalog Value,
    flow_owner: Value,
    catalog: &'catalog Value,
}

impl<'catalog> Template<'catalog> {
    pub(crate) fn new(catalog: &'catalog Value, guid: &str) -> TestResult<Self> {
        assert!(
            catalog.get("contractTemplates").is_none(),
            "no legacy root inventory"
        );
        let definitions = catalog
            .pointer("/definitions/contractTemplates/rows")
            .and_then(Value::as_array)
            .ok_or("canonical physical Template owners")?;
        let found: Vec<_> = definitions
            .iter()
            .filter(|row| row.pointer("/guid/value") == Some(&json!(guid)))
            .collect();
        assert_eq!(
            found.len(),
            1,
            "controlled original identity has one physical D owner"
        );
        let definition = *found.first().ok_or("actual physical Template definition")?;
        let physical = json!({
            "inputRef":definition.get("inputRef").ok_or("actual input navigation")?,
            "recordIndex":raw::present(definition.get("recordIndex").ok_or("original Record index")?)?,
        });
        let projections = catalog
            .pointer("/configurations/contractTemplates/rows")
            .and_then(Value::as_array)
            .ok_or("permanent Template context configurations")?;
        let matching: Vec<_> = projections
            .iter()
            .filter(|row| row.get("template") == Some(&physical))
            .collect();
        assert_eq!(
            matching.len(),
            1,
            "one contextual projection for this physical owner"
        );
        let projection = *matching
            .first()
            .ok_or("actual permanent Template projection")?;
        let flow_owner = json!({
            "type":"contractTemplate", "inputRef":physical.get("inputRef"),
            "recordIndex":physical.get("recordIndex"),
            "recordGuid":raw::present(definition.get("guid").ok_or("actual original GUID")?)?,
        });
        Ok(Self {
            owner: Owner::new(definition)?,
            projection,
            definition,
            flow_owner,
            catalog,
        })
    }

    pub(crate) fn triggers(&self) -> TestResult<&'catalog [Value]> {
        let rows = self
            .catalog
            .pointer("/configurations/missionFlows/rows")
            .and_then(Value::as_array)
            .ok_or("canonical Flow configurations")?;
        let matching: Vec<_> = rows
            .iter()
            .filter(|row| row.get("owner") == Some(&self.flow_owner))
            .collect();
        assert_eq!(
            matching.len(),
            1,
            "one actual Flow declaration in this controlled source"
        );
        let row = *matching.first().ok_or("actual Flow configuration")?;
        raw::array(row, "triggers")
    }

    pub(crate) fn condition(&self) -> TestResult<&'catalog Value> {
        let triggers = self.triggers()?;
        assert_eq!(triggers.len(), 1, "one actual original trigger");
        let trigger = self
            .owner
            .follow(triggers.first().ok_or("actual trigger link")?)?;
        let reference = trigger
            .pointer("/trigger/condition")
            .ok_or("actual trigger condition binding")?;
        self.owner.follow(reference)
    }

    pub(crate) fn field(&self, node: &Value, name: &str) -> TestResult<&'catalog Value> {
        Context::field(self, node, name)
    }

    pub(crate) fn bound(&self, reference: &Value) -> TestResult<&'catalog Value> {
        Context::bound(self, reference)
    }
}

/// Borrowed access to one actual mission owner and its contextual row.
pub(crate) trait Context<'catalog> {
    fn owner(&self) -> &Owner<'catalog>;
    fn projection(&self) -> &'catalog Value;

    fn field(&self, node: &Value, name: &str) -> TestResult<&'catalog Value> {
        struct_field(self.owner(), node, name)
    }

    fn bound(&self, reference: &Value) -> TestResult<&'catalog Value> {
        self.owner().follow(
            self.owner()
                .field(reference)?
                .get("value")
                .ok_or("original source field value")?,
        )
    }
}

impl<'catalog> Context<'catalog> for Template<'catalog> {
    fn owner(&self) -> &Owner<'catalog> {
        &self.owner
    }
    fn projection(&self) -> &'catalog Value {
        self.projection
    }
}

pub(crate) struct BrokerContext<'catalog> {
    owner: Owner<'catalog>,
    projection: &'catalog Value,
}

impl<'catalog> BrokerContext<'catalog> {
    pub(crate) fn new(catalog: &'catalog Value, guid: &str) -> TestResult<Self> {
        let definition = crate::broker_flow_graph::definition(catalog, guid)?;
        let projection = crate::broker_flow_graph::projection(catalog, definition)?;
        Ok(Self {
            owner: Owner::new(definition)?,
            projection,
        })
    }

    pub(crate) fn root(&self, name: &str) -> TestResult<&'catalog Value> {
        root_field(&self.owner, name)
    }
}

impl<'catalog> Context<'catalog> for BrokerContext<'catalog> {
    fn owner(&self) -> &Owner<'catalog> {
        &self.owner
    }
    fn projection(&self) -> &'catalog Value {
        self.projection
    }
}

pub(crate) fn struct_field<'catalog>(
    owner: &Owner<'catalog>,
    node: &Value,
    name: &str,
) -> TestResult<&'catalog Value> {
    let fields = raw::array(raw::payload(node, "struct")?, "fields")?;
    let found: Vec<_> = fields
        .iter()
        .filter(|field| field.pointer("/name/value") == Some(&json!(name)))
        .collect();
    assert_eq!(
        found.len(),
        1,
        "one original declaration in this controlled Struct"
    );
    owner.follow(
        found
            .first()
            .ok_or("actual original field")?
            .get("value")
            .ok_or("actual original field value link")?,
    )
}

pub(crate) fn root_field<'catalog>(
    owner: &Owner<'catalog>,
    name: &str,
) -> TestResult<&'catalog Value> {
    let roots = raw::array(owner.source, "roots")?;
    let found: Vec<_> = roots
        .iter()
        .filter(|field| field.pointer("/name/value") == Some(&json!(name)))
        .collect();
    assert_eq!(
        found.len(),
        1,
        "one actual controlled Broker root declaration {name}"
    );
    owner.follow(
        found
            .first()
            .ok_or("actual original Broker root")?
            .get("value")
            .ok_or("actual root value navigation")?,
    )
}

pub(crate) fn condition(node: &Value) -> TestResult<&Value> {
    let meaning = node
        .get("condition")
        .ok_or("required nullable ConditionMeaning")?;
    assert!(
        !meaning.is_null(),
        "the original Struct has an actual condition role"
    );
    Ok(meaning)
}
