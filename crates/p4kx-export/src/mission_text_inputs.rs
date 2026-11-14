// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{StructValue, Value};

use crate::CatalogError;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValuePart};

#[path = "mission_text_evidence.rs"]
mod evidence;
#[path = "mission_item_token_choice.rs"]
mod item_choice;
#[path = "mission_item_token_texts.rs"]
mod item_texts;
#[path = "mission_text_organization.rs"]
mod organization;
#[path = "mission_text_token_inputs.rs"]
mod token_inputs;

pub(crate) use self::evidence::{Consulted, Decisions, MissionKey};
pub(crate) use self::item_choice::{MissionItemChoice, MissionItemGap};
pub(crate) use self::item_texts::MissionItemTexts;
pub(crate) use self::organization::OrganizationTexts;
pub(crate) use self::token_inputs::TokenChoice;

use self::token_inputs::Tokens;

use self::evidence::{Fact, Fields};

/// A selected original key and the decision inputs of the reader that chose it.
#[derive(Debug)]
pub(crate) struct TextSelection<'source> {
    pub(crate) key: Option<MissionKey<'source>>,
    pub(crate) decisions: Decisions,
}

/// The original layer supplying a generator's text settings.
#[derive(Clone, Copy)]
pub(crate) enum LayerKind {
    Handler,
    Contract,
    SubContract,
}

/// The title, description and token inputs of one actual generator layer.
#[derive(Debug)]
pub(crate) struct LayerTexts<'source> {
    pub(crate) parameters: Decisions,
    pub(crate) title: TextSelection<'source>,
    pub(crate) description: TextSelection<'source>,
    pub(crate) tokens: Tokens<'source>,
}

impl<'source> LayerTexts<'source> {
    pub(crate) fn read(
        original: &OriginalFlowFields<'_, '_, 'source>,
        origins: &CatalogOrigins,
        owner: &StructValue<'source>,
        layer: LayerKind,
    ) -> Result<Self, CatalogError> {
        let fields = Fields::new(original, origins)?;
        let owner = fields.owner(owner)?;
        let mut parameters = fields.decisions();
        let name = match layer {
            LayerKind::Handler => "contractParams",
            LayerKind::Contract | LayerKind::SubContract => "paramOverrides",
        };
        let selected = child(&fields, owner, name, &mut parameters)?;
        let params = if matches!(layer, LayerKind::SubContract) {
            selected.or(owner)
        } else {
            selected
        };
        let title = string_override(&fields, params, "Title")?;
        let description = string_override(&fields, params, "Description")?;
        let tokens = if let Some(params) = params {
            Tokens::read(&fields, Some(params), "propertyOverrides")?
        } else {
            Tokens::empty(&fields)
        };
        Ok(Self {
            parameters,
            title,
            description,
            tokens,
        })
    }
}

/// The original text inputs read from the actual Template dependency.
#[derive(Debug)]
pub(crate) struct TemplateTexts<'source> {
    pub(crate) title: TextSelection<'source>,
    pub(crate) tokens: Tokens<'source>,
}

impl<'source> TemplateTexts<'source> {
    pub(crate) fn read(
        original: &OriginalFlowFields<'_, '_, 'source>,
        origins: &CatalogOrigins,
    ) -> Result<Self, CatalogError> {
        let fields = Fields::new(original, origins)?;
        Ok(Self {
            title: template_title(&fields)?,
            tokens: Tokens::read(&fields, None, "contractProperties")?,
        })
    }
}

/// The title and description read from the actual Broker dependency.
#[derive(Debug)]
pub(crate) struct BrokerTexts<'source> {
    pub(crate) title: TextSelection<'source>,
    pub(crate) description: TextSelection<'source>,
}

impl<'source> BrokerTexts<'source> {
    pub(crate) fn read(
        original: &OriginalFlowFields<'_, '_, 'source>,
        origins: &CatalogOrigins,
    ) -> Result<Self, CatalogError> {
        let fields = Fields::new(original, origins)?;
        Ok(Self {
            title: root_key(&fields, "title")?,
            description: root_key(&fields, "description")?,
        })
    }
}

/// Authored Template and Broker selectors of one physical contract.
pub(crate) struct TextDependencyLinks {
    pub(crate) template: Decisions,
    pub(crate) broker: Decisions,
}

impl TextDependencyLinks {
    pub(crate) fn read<'source>(
        original: &OriginalFlowFields<'_, '_, 'source>,
        origins: &CatalogOrigins,
        contract: &StructValue<'source>,
    ) -> Result<Self, CatalogError> {
        let fields = Fields::new(original, origins)?;
        let owner = fields
            .owner(contract)?
            .ok_or(CatalogError::MissionTextPlan)?;
        let mut template = fields.decisions();
        fields.field(Some(owner), "template", &mut template)?;
        let mut broker = fields.decisions();
        fields.field(Some(owner), "missionBrokerEntry", &mut broker)?;
        Ok(Self { template, broker })
    }
}

fn root_key<'source>(
    fields: &Fields<'_, '_, '_, 'source>,
    name: &str,
) -> Result<TextSelection<'source>, CatalogError> {
    let mut decisions = fields.decisions();
    let key = fields
        .field(None, name, &mut decisions)?
        .map(|fact| fields.key(fact))
        .transpose()?
        .flatten();
    Ok(TextSelection { key, decisions })
}

fn template_title<'source>(
    fields: &Fields<'_, '_, '_, 'source>,
) -> Result<TextSelection<'source>, CatalogError> {
    let mut decisions = fields.decisions();
    let Some(display) = child(fields, None, "contractDisplayInfo", &mut decisions)? else {
        return Ok(TextSelection {
            key: None,
            decisions,
        });
    };
    let Some(strings) = fields.field(Some(display), "displayString", &mut decisions)? else {
        return Ok(TextSelection {
            key: None,
            decisions,
        });
    };
    let first = fields.items(strings)?.into_iter().next();
    let key = match first {
        Some(fact) if matches!(fact.value(), Value::Struct(_)) => {
            fields.field(Some(fact), "value", &mut decisions)?
        }
        Some(fact) => {
            fields.record(fact, OriginalValuePart::Value, &mut decisions)?;
            Some(fact)
        }
        None => None,
    }
    .map(|fact| fields.key(fact))
    .transpose()?
    .flatten();
    Ok(TextSelection { key, decisions })
}

fn string_override<'fields, 'scope, 'read, 'source>(
    fields: &Fields<'fields, 'scope, 'read, 'source>,
    params: Option<Fact<'fields, 'scope, 'read, 'source>>,
    name: &str,
) -> Result<TextSelection<'source>, CatalogError> {
    let mut decisions = fields.decisions();
    if let Some(params) = params
        && let Some(list) = fields.field(Some(params), "stringParamOverrides", &mut decisions)?
    {
        for candidate in fields.items(list)? {
            if !matches!(candidate.value(), Value::Struct(_)) {
                fields.record(candidate, OriginalValuePart::Value, &mut decisions)?;
                continue;
            }
            let param = fields.field(Some(candidate), "param", &mut decisions)?;
            if param.and_then(text) == Some(name) {
                let key = fields
                    .field(Some(candidate), "value", &mut decisions)?
                    .map(|fact| fields.key(fact))
                    .transpose()?
                    .flatten();
                return Ok(TextSelection { key, decisions });
            }
        }
    }
    Ok(TextSelection {
        key: None,
        decisions,
    })
}

fn child<'fields, 'scope, 'read, 'source>(
    fields: &Fields<'fields, 'scope, 'read, 'source>,
    parent: Option<Fact<'fields, 'scope, 'read, 'source>>,
    name: &str,
    decisions: &mut Decisions,
) -> Result<Option<Fact<'fields, 'scope, 'read, 'source>>, CatalogError> {
    fields
        .field(parent, name, decisions)?
        .map(|fact| fields.selected(fact, decisions))
        .transpose()
        .map(Option::flatten)
}

const fn text<'source>(fact: Fact<'_, '_, '_, 'source>) -> Option<&'source str> {
    match fact.value() {
        Value::String(text) | Value::Enum(text) if !text.is_empty() => Some(*text),
        _ => None,
    }
}
