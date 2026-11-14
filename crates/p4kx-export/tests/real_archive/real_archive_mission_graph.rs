// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};

use serde_json::Value;

pub(super) type Result<T> = std::result::Result<T, String>;

#[derive(Clone, Copy)]
pub(crate) enum Family {
    Template,
    Broker,
}

impl Family {
    const fn key(self) -> &'static str {
        match self {
            Self::Template => "contractTemplates",
            Self::Broker => "missionBrokerEntries",
        }
    }

    const fn link(self) -> &'static str {
        match self {
            Self::Template => "template",
            Self::Broker => "broker",
        }
    }
}

pub(crate) struct Mission<'catalog> {
    pub(crate) definition: &'catalog Value,
    pub(crate) projection: &'catalog Value,
    arena: &'catalog Value,
    registrations: HashMap<usize, &'catalog Value>,
}

pub(crate) fn contexts(catalog: &Value, family: Family) -> Result<Vec<Mission<'_>>> {
    let definitions = table(catalog, "definitions", family)?;
    let mut by_identity = HashMap::new();
    for definition in definitions {
        let physical = (
            text(definition, "/inputRef")?,
            integer(definition, "/recordIndex/value")?,
        );
        if by_identity.insert(physical, definition).is_some() {
            return Err(format!(
                "{} repeats an original physical owner",
                family.key()
            ));
        }
    }
    let mut result = Vec::new();
    for projection in table(catalog, "configurations", family)? {
        let link = projection
            .get(family.link())
            .ok_or("required physical owner link")?;
        let identity = (text(link, "/inputRef")?, integer(link, "/recordIndex")?);
        let definition = by_identity
            .remove(&identity)
            .ok_or("one original D owner for each C context")?;
        if matches!(family, Family::Broker)
            && (text(link, "/type")? != "missionBrokerEntry"
                || text(link, "/recordGuid")? != guid(definition)?)
        {
            return Err(
                "a Broker configuration names the same physical family and original GUID".into(),
            );
        }
        result.push(Mission::new(definition, projection)?);
    }
    Ok(result)
}

fn table<'catalog>(
    catalog: &'catalog Value,
    section: &str,
    family: Family,
) -> Result<&'catalog [Value]> {
    let path = format!("/{section}/{}/rows", family.key());
    catalog
        .pointer(&path)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or(path)
}

impl<'catalog> Mission<'catalog> {
    fn new(definition: &'catalog Value, projection: &'catalog Value) -> Result<Self> {
        let arena = definition.get("source").ok_or("original arena")?;
        let mut registrations = HashMap::new();
        for row in array(projection, "spawnPools")? {
            let source = position(row, "/source/valueIndex")?;
            let result = row.get("result").ok_or("actual registration result")?;
            if registrations.insert(source, result).is_some() {
                return Err("one registration per original spawn occurrence".into());
            }
        }
        Ok(Self {
            definition,
            projection,
            arena,
            registrations,
        })
    }

    pub(crate) fn root(&self, name: &str) -> Result<Option<&'catalog Value>> {
        self.named(array(self.arena, "roots")?, name)
    }

    pub(crate) fn member(
        &self,
        node: &'catalog Value,
        name: &str,
    ) -> Result<Option<&'catalog Value>> {
        let Some(payload) = payload(node)? else {
            return Ok(None);
        };
        if text(payload, "/type")? != "struct" {
            return Err(format!("expected an original Struct before reading {name}"));
        }
        self.named(array(payload, "fields")?, name)
    }

    fn named(&self, fields: &'catalog [Value], name: &str) -> Result<Option<&'catalog Value>> {
        fields
            .iter()
            .find(|field| field.pointer("/name/value").and_then(Value::as_str) == Some(name))
            .map(|field| self.follow(field.get("value").ok_or("original field value link")?))
            .transpose()
    }

    pub(crate) fn first_struct(&self, node: &'catalog Value) -> Result<Option<&'catalog Value>> {
        match payload(node)?
            .map(|value| text(value, "/type"))
            .transpose()?
        {
            Some("struct") => Ok(Some(node)),
            Some("array") => {
                for child in self.elements(node)? {
                    if payload(child)?
                        .map(|value| text(value, "/type"))
                        .transpose()?
                        == Some("struct")
                    {
                        return Ok(Some(child));
                    }
                }
                Ok(None)
            }
            _ => Ok(None),
        }
    }

    pub(crate) fn field(&self, reference: &Value) -> Result<&'catalog Value> {
        let field = match text(reference, "/type")? {
            "root" => array(self.arena, "roots")?.get(position(reference, "/rootFieldIndex")?),
            "field" => {
                let parent = self.follow(
                    reference
                        .get("parent")
                        .ok_or("actual original Struct parent")?,
                )?;
                let payload = payload(parent)?.ok_or("actual readable Struct payload")?;
                if text(payload, "/type")? != "struct" {
                    return Err("an original field link must name a Struct parent".into());
                }
                array(payload, "fields")?.get(position(reference, "/fieldIndex")?)
            }
            kind => return Err(format!("unknown original field reference {kind}")),
        }
        .ok_or("owner-local original declaration")?;
        self.follow(
            field
                .get("value")
                .ok_or("original declaration value link")?,
        )
    }

    pub(crate) fn follow(&self, reference: &Value) -> Result<&'catalog Value> {
        array(self.arena, "values")?
            .get(position(reference, "/valueIndex")?)
            .ok_or_else(|| "owner-local original value".into())
    }

    pub(crate) fn elements(&self, node: &'catalog Value) -> Result<Vec<&'catalog Value>> {
        let Some(payload) = payload(node)? else {
            return Ok(Vec::new());
        };
        if text(payload, "/type")? != "array" {
            return Err("expected an original array".into());
        }
        array(payload, "elements")?
            .iter()
            .map(|link| self.follow(link))
            .collect()
    }

    pub(crate) fn pools(&self) -> Result<Vec<Pool<'catalog>>> {
        array(self.projection, "poolBindings")?
            .iter()
            .map(|binding| {
                let selected = binding
                    .get("registration")
                    .ok_or("actual selected registration reference")?;
                let registration = self
                    .registration(selected)?
                    .ok_or("actual source registration")?;
                let (family, guid) = registration
                    .identifier()?
                    .ok_or("the selected registration is available")?;
                self.field(binding.get("variable").ok_or("actual selected variable")?)?;
                Ok(Pool {
                    family,
                    guid,
                    level: text(binding, "/level")?,
                })
            })
            .collect()
    }

    pub(crate) fn registration(&self, reference: &Value) -> Result<Option<Registration<'catalog>>> {
        self.follow(reference)?;
        Ok(self
            .registrations
            .get(&position(reference, "/valueIndex")?)
            .copied()
            .map(|value| Registration { value }))
    }

    pub(crate) fn parents(&self) -> Result<()> {
        let objectives = array(self.projection, "objectives")?;
        let mut seen = HashSet::new();
        for objective in objectives {
            let source = objective.get("source").ok_or("actual physical objective")?;
            self.follow(source)?;
            let source = position(source, "/valueIndex")?;
            let parent = objective
                .get("parent")
                .ok_or("required nullable physical parent")?;
            if !parent.is_null() && !seen.contains(&position(parent, "/valueIndex")?) {
                return Err(
                    "the original objective parent must precede its child in the same owner".into(),
                );
            }
            if !seen.insert(source) {
                return Err("one projection per actual original objective".into());
            }
        }
        Ok(())
    }
}

pub(crate) struct Pool<'catalog> {
    pub(crate) family: &'catalog str,
    pub(crate) guid: &'catalog str,
    pub(crate) level: &'catalog str,
}

pub(crate) struct Registration<'catalog> {
    value: &'catalog Value,
}

impl<'catalog> Registration<'catalog> {
    pub(crate) fn identifier(&self) -> Result<Option<(&'catalog str, &'catalog str)>> {
        let family = text(self.value, "/type")?;
        let guid = match family {
            "ships" | "npcs" | "entities" => text(self.value, "/pool/value")?,
            "location" => text(self.value, "/query/value")?,
            "unavailable" => return Ok(None),
            _ => return Err(format!("unknown contextual registration {family}")),
        };
        Ok(Some((family, guid)))
    }
}

pub(crate) fn guid(definition: &Value) -> Result<&str> {
    text(definition, "/guid/value")
}

pub(crate) fn reference(node: &Value) -> Result<Option<&str>> {
    if node.pointer("/originalError/type").and_then(Value::as_str) == Some("danglingReference") {
        return text(node, "/originalError/guid/value").map(Some);
    }
    let Some(payload) = payload(node)? else {
        return Ok(None);
    };
    let kind = text(payload, "/type")?;
    if kind != "recordReference" {
        return Err(format!("expected an original record reference, got {kind}"));
    }
    text(payload, "/value/guid/value").map(Some)
}

fn payload(node: &Value) -> Result<Option<&Value>> {
    let value = node
        .pointer("/value/value")
        .ok_or("the original source datum carries a value slot")?;
    match text(node, "/value/state")? {
        "present" if !value.is_null() => Ok(Some(value)),
        "null" | "unreadable" | "invalid" | "absent" if value.is_null() => Ok(None),
        state => Err(format!("inconsistent original datum state {state}")),
    }
}

pub(crate) fn array<'value>(node: &'value Value, key: &str) -> Result<&'value [Value]> {
    node.get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| format!("required array {key}"))
}

pub(crate) fn text<'value>(node: &'value Value, path: &str) -> Result<&'value str> {
    node.pointer(path)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("required text at {path}"))
}

fn integer(node: &Value, path: &str) -> Result<u64> {
    node.pointer(path)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("required integer at {path}"))
}

pub(crate) fn position(reference: &Value, path: &str) -> Result<usize> {
    let original = text(reference, path)?;
    let index: usize = original
        .parse()
        .map_err(|error| format!("{path}: {error}"))?;
    if index.to_string() != original {
        return Err(format!("{path} is not a canonical decimal index"));
    }
    Ok(index)
}
