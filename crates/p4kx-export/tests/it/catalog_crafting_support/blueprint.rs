// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::error::Error;

use serde_json::Value;

type TestResult<T> = Result<T, Box<dyn Error>>;

/// Navigates the actual source and contextual rows of one physical Blueprint.
pub(crate) struct Blueprint<'catalog> {
    catalog: &'catalog Value,
    pub(crate) definition: &'catalog Value,
    pub(crate) presentation: &'catalog Value,
}

pub(crate) fn definitions(catalog: &Value) -> TestResult<&[Value]> {
    rows(catalog, "/definitions/blueprints/rows")
}

pub(crate) fn by_guid(catalog: &Value, guid: u128) -> TestResult<Blueprint<'_>> {
    let wanted = super::text(guid);
    let definition = definitions(catalog)?
        .iter()
        .find(|definition| {
            definition.pointer("/guid/value").and_then(Value::as_str) == Some(wanted.as_str())
        })
        .ok_or_else(|| format!("Blueprint definition {wanted}"))?;
    Blueprint::new(catalog, definition)
}

pub(crate) fn by_index(catalog: &Value, index: usize) -> TestResult<Blueprint<'_>> {
    Blueprint::new(
        catalog,
        definitions(catalog)?
            .get(index)
            .ok_or("Blueprint definition index")?,
    )
}

impl<'catalog> Blueprint<'catalog> {
    fn new(catalog: &'catalog Value, definition: &'catalog Value) -> TestResult<Self> {
        let presentation = rows(catalog, "/projections/blueprintPresentation/rows")?
            .iter()
            .find(|row| same_owner(definition, &row["blueprint"]))
            .ok_or("physical Blueprint presentation")?;
        Ok(Self {
            catalog,
            definition,
            presentation,
        })
    }

    pub(crate) fn tiers(&self) -> TestResult<Vec<&'catalog Value>> {
        Ok(rows(self.catalog, "/configurations/blueprintTiers/rows")?
            .iter()
            .filter(|row| same_owner(self.definition, &row["blueprint"]))
            .collect())
    }

    pub(crate) fn node(&self, source: &Value) -> TestResult<&'catalog Value> {
        let index = source
            .get("valueIndex")
            .and_then(Value::as_str)
            .ok_or("source occurrence")?
            .parse::<usize>()?;
        rows(self.definition, "/source/values")?
            .get(index)
            .ok_or_else(|| "source occurrence range".into())
    }

    pub(crate) fn root(&self, name: &str) -> TestResult<&'catalog Value> {
        self.node(named_field_ref(
            rows(self.definition, "/source/roots")?,
            name,
        )?)
    }

    pub(crate) fn field(
        &self,
        structure: &'catalog Value,
        name: &str,
    ) -> TestResult<&'catalog Value> {
        self.node(field_ref(structure, name)?)
    }

    pub(crate) fn ingredient(
        &self,
        index: usize,
    ) -> TestResult<(&'catalog Value, Option<&'catalog Value>)> {
        let ingredients = rows(self.presentation, "/ingredients")?;
        let ingredient = ingredients.get(index).ok_or("ingredient index")?;
        let tiers = self.tiers()?;
        let tier = tiers.first().copied().ok_or("first tier")?;
        let leaf = cost(tier, ingredient.get("cost").ok_or("ingredient cost")?)?;
        let slot = ingredient
            .get("slot")
            .filter(|slot| !slot.is_null())
            .map(|slot| cost(tier, slot))
            .transpose()?;
        Ok((leaf, slot))
    }

    pub(crate) fn debug_name(&self, selection: &Value) -> TestResult<&'catalog Value> {
        let node = self.node(
            selection
                .pointer("/meaning/debugName")
                .ok_or("selection debug name occurrence")?,
        )?;
        node.pointer("/value/value/value/sourceText")
            .ok_or_else(|| "selection source debug name".into())
    }

    pub(crate) fn game_key(&self, cell: &Value) -> TestResult<&'catalog Value> {
        let id = cell["value"].as_str().ok_or("game text reference")?;
        let row = rows(self.catalog, "/texts/gameTexts")?
            .iter()
            .find(|row| row.pointer("/id/value").and_then(Value::as_str) == Some(id))
            .ok_or("actual game text row")?;
        row.pointer("/key/value")
            .ok_or_else(|| "actual game text key".into())
    }
}

fn same_owner(definition: &Value, owner: &Value) -> bool {
    let Some(input) = definition.get("inputRef") else {
        return false;
    };
    let Some(index) = definition
        .pointer("/recordIndex/value")
        .and_then(Value::as_u64)
    else {
        return false;
    };
    owner.get("inputRef") == Some(input)
        && owner.get("recordIndex").and_then(Value::as_u64) == Some(index)
}

fn rows<'document>(value: &'document Value, pointer: &str) -> TestResult<&'document [Value]> {
    value
        .pointer(pointer)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| pointer.to_owned().into())
}

pub(crate) fn cost<'tier>(tier: &'tier Value, source: &Value) -> TestResult<&'tier Value> {
    let index = source
        .get("valueIndex")
        .and_then(Value::as_str)
        .ok_or("cost occurrence")?;
    rows(tier, "/costs")?
        .iter()
        .find(|cost| cost.pointer("/source/valueIndex").and_then(Value::as_str) == Some(index))
        .ok_or_else(|| format!("interpreted cost occurrence {index}").into())
}

pub(crate) fn field_ref<'structure>(
    structure: &'structure Value,
    name: &str,
) -> TestResult<&'structure Value> {
    named_field_ref(rows(structure, "/value/value/fields")?, name)
}

fn named_field_ref<'document>(
    fields: &'document [Value],
    name: &str,
) -> TestResult<&'document Value> {
    let field = fields
        .iter()
        .find(|field| field.pointer("/name/value").and_then(Value::as_str) == Some(name))
        .ok_or_else(|| format!("original field {name}"))?;
    field
        .get("value")
        .ok_or_else(|| "original occurrence".into())
}

/// Resolves actual value selectors of one physical Blueprint from a sourced choice.
pub(crate) fn selected_ordinals<'document>(
    catalog: &'document Value,
    cell: &'document Value,
    record_index: u64,
) -> TestResult<BTreeSet<&'document str>> {
    let ids: BTreeSet<_> = rows(cell, "/sourceUsesRefs")?
        .iter()
        .map(|id| id.as_str().ok_or("source use id"))
        .collect::<Result<_, _>>()?;
    let index = record_index.to_string();
    let mut found = BTreeSet::new();
    for used in rows(catalog, "/provenance/uses")? {
        let id = used["id"].as_str().ok_or("inventory use id")?;
        let location = &used["location"];
        if ids.contains(id)
            && used["role"] == "selector"
            && used["scope"] == "payload"
            && location["namespace"] == "dataforgeValue"
            && location["field"] == "$value"
            && location.pointer("/indices/0").and_then(Value::as_str) == Some(index.as_str())
        {
            found.insert(
                location
                    .pointer("/indices/1")
                    .and_then(Value::as_str)
                    .ok_or("actual value ordinal")?,
            );
        }
    }
    Ok(found)
}
