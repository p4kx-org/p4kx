// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_extraction::{Guid, Value};

use super::evidence::{Consulted, Decisions, Fact, Fields, MissionKey};
use super::item_choice::{self, MissionItemChoice};
use crate::CatalogError;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_read::original::OriginalValuePart;
use crate::mission_title::AsciiKey;

/// The original first matching property's text, runtime class or unsupported state.
#[derive(Debug)]
pub(crate) enum TokenChoice<'source> {
    Key(MissionKey<'source>),
    Organization(Guid),
    MissionItem(MissionItemChoice),
    Runtime(&'static str),
    Unknown,
}

#[derive(Debug)]
struct Candidate<'source> {
    prefix: usize,
    value: TokenChoice<'source>,
    inputs: Decisions,
}

/// The original names in order and the first applicable candidate for each token.
#[derive(Debug)]
pub(crate) struct Tokens<'source> {
    names: Decisions,
    by_name: HashMap<AsciiKey<'source>, usize>,
    candidates: Vec<Candidate<'source>>,
}

impl<'source> Tokens<'source> {
    pub(super) fn empty(fields: &Fields<'_, '_, '_, 'source>) -> Self {
        Self {
            names: fields.decisions(),
            by_name: HashMap::new(),
            candidates: Vec::new(),
        }
    }

    pub(super) fn read<'fields, 'scope, 'read>(
        fields: &Fields<'fields, 'scope, 'read, 'source>,
        parent: Option<Fact<'fields, 'scope, 'read, 'source>>,
        name: &str,
    ) -> Result<Self, CatalogError> {
        let mut tokens = Self::empty(fields);
        if let Some(list) = fields.field(parent, name, &mut tokens.names)? {
            for candidate in fields.items(list)? {
                tokens.read_candidate(fields, candidate)?;
            }
        }
        Ok(tokens)
    }

    fn read_candidate<'fields, 'scope, 'read>(
        &mut self,
        fields: &Fields<'fields, 'scope, 'read, 'source>,
        candidate: Fact<'fields, 'scope, 'read, 'source>,
    ) -> Result<(), CatalogError> {
        if !matches!(candidate.value(), Value::Struct(_)) {
            return fields.record(candidate, OriginalValuePart::Value, &mut self.names);
        }
        let Some(name) = fields
            .field(Some(candidate), "extendedTextToken", &mut self.names)?
            .and_then(super::text)
        else {
            return Ok(());
        };
        let token = AsciiKey(name);
        if self.by_name.contains_key(&token) {
            return Ok(());
        }
        let mut inputs = fields.decisions();
        let value = fields
            .field(Some(candidate), "value", &mut inputs)?
            .map(|value| fields.selected(value, &mut inputs))
            .transpose()?
            .flatten();
        let value = value
            .map(|value| read_choice(fields, value, &mut inputs))
            .transpose()?
            .unwrap_or(TokenChoice::Unknown);
        self.by_name
            .try_reserve(1)
            .map_err(CatalogError::MissionTextAllocation)?;
        self.candidates
            .try_reserve(1)
            .map_err(CatalogError::MissionTextAllocation)?;
        self.by_name.insert(token, self.candidates.len());
        self.candidates.push(Candidate {
            prefix: self.names.len(),
            value,
            inputs,
        });
        Ok(())
    }

    pub(crate) fn select<'read>(
        &'read self,
        token: &str,
        origins: &CatalogOrigins,
        inputs: &mut Consulted<'read>,
    ) -> Result<Option<&'read TokenChoice<'source>>, CatalogError> {
        let Some(index) = self.by_name.get(&AsciiKey(token)).copied() else {
            inputs.all(&self.names, origins)?;
            return Ok(None);
        };
        let candidate = self
            .candidates
            .get(index)
            .ok_or(CatalogError::MissionTextPlan)?;
        inputs.prefix(&self.names, candidate.prefix, origins)?;
        inputs.all(&candidate.inputs, origins)?;
        Ok(Some(&candidate.value))
    }
}

fn read_choice<'fields, 'scope, 'read, 'source>(
    fields: &Fields<'fields, 'scope, 'read, 'source>,
    value: Fact<'fields, 'scope, 'read, 'source>,
    inputs: &mut Decisions,
) -> Result<TokenChoice<'source>, CatalogError> {
    let Value::Struct(source) = value.value() else {
        return Err(CatalogError::MissionTextPlan);
    };
    Ok(match source.type_name() {
        "MissionPropertyValue_StringHash" => {
            first_key(fields, value, inputs)?.map_or(TokenChoice::Unknown, TokenChoice::Key)
        }
        "MissionPropertyValue_MissionItem" => {
            TokenChoice::MissionItem(item_choice::select(fields, value, inputs)?)
        }
        "MissionPropertyValue_Organization" => first_organization(fields, value, inputs)?
            .map_or(TokenChoice::Unknown, TokenChoice::Organization),
        "MissionPropertyValue_Location"
        | "MissionPropertyValue_Locations"
        | "MissionPropertyValue_ScopedLocation" => TokenChoice::Runtime("LOCATION"),
        "MissionPropertyValue_AIName" | "MissionPropertyValue_NPCSpawnDescriptions" => {
            TokenChoice::Runtime("TARGET")
        }
        "MissionPropertyValue_ShipSpawnDescriptions" => TokenChoice::Runtime("SHIP"),
        _ => TokenChoice::Unknown,
    })
}

fn first_key<'fields, 'scope, 'read, 'source>(
    fields: &Fields<'fields, 'scope, 'read, 'source>,
    value: Fact<'fields, 'scope, 'read, 'source>,
    inputs: &mut Decisions,
) -> Result<Option<MissionKey<'source>>, CatalogError> {
    let Some(list) = fields.field(Some(value), "options", inputs)? else {
        return Ok(None);
    };
    for option in fields.items(list)? {
        if !matches!(option.value(), Value::Struct(_)) {
            fields.record(option, OriginalValuePart::Value, inputs)?;
            continue;
        }
        if let Some(key) = fields
            .field(Some(option), "textId", inputs)?
            .map(|key| fields.key(key))
            .transpose()?
            .flatten()
        {
            return Ok(Some(key));
        }
    }
    Ok(None)
}

fn first_organization<'fields, 'scope, 'read, 'source>(
    fields: &Fields<'fields, 'scope, 'read, 'source>,
    value: Fact<'fields, 'scope, 'read, 'source>,
    inputs: &mut Decisions,
) -> Result<Option<Guid>, CatalogError> {
    let Some(list) = fields.field(Some(value), "matchConditions", inputs)? else {
        return Ok(None);
    };
    for condition in fields.items(list)? {
        if !matches!(condition.value(), Value::Struct(_)) {
            fields.record(condition, OriginalValuePart::Value, inputs)?;
            continue;
        }
        let Some(organizations) = fields.field(Some(condition), "organizations", inputs)? else {
            continue;
        };
        for candidate in fields.items(organizations)? {
            fields.record(candidate, OriginalValuePart::Value, inputs)?;
            if let Value::Reference(target) = candidate.value()
                && target.location().is_none()
            {
                return Ok(Some(target.guid()));
            }
        }
    }
    Ok(None)
}
