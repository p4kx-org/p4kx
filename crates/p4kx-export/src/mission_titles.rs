// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::CancelToken;
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_projection::CatalogProjection;
use crate::mission_generator::ContractOccurrence;
use crate::mission_index::MissionIndex;
use crate::mission_text_inputs::{
    BrokerTexts, Consulted, Decisions, LayerTexts, MissionItemChoice, MissionItemGap,
    TemplateTexts, TextDependencyLinks, TextSelection, TokenChoice,
};
use crate::mission_title;
use crate::mission_title_origin::{
    self, ProjectedMissionText, ResolvedToken, SelectedItemKey, SelectedMissionText,
};
use crate::schema_fields::every_field_required;

/// The title and description of the same physically identified projected contract.
#[derive(Debug, Serialize, JsonSchema)]
#[schemars(transform = every_field_required)]
pub(crate) struct MissionTitles<'source> {
    pub(crate) contract: ContractOccurrence,
    #[schemars(extend("x-gameText" = true))]
    pub(crate) title: ProjectedMissionText<'source>,
    #[schemars(extend("x-gameText" = true))]
    pub(crate) description: ProjectedMissionText<'source>,
}

pub(crate) struct Layers<'read, 'source> {
    pub(crate) sub: Option<&'read LayerTexts<'source>>,
    pub(crate) contract: &'read LayerTexts<'source>,
    pub(crate) handler: &'read LayerTexts<'source>,
}

impl<'read, 'source> Layers<'read, 'source> {
    fn iter(&self) -> impl Iterator<Item = &'read LayerTexts<'source>> {
        self.sub.into_iter().chain([self.contract, self.handler])
    }
}

pub(crate) struct Dependencies<'read, 'source> {
    pub(crate) template: Option<&'read TemplateTexts<'source>>,
    pub(crate) broker: Option<&'read BrokerTexts<'source>>,
    pub(crate) links: &'read TextDependencyLinks,
}

pub(crate) struct Inputs<'read, 'catalog, 'source> {
    pub(crate) projection: &'read CatalogProjection<'catalog, 'source>,
    pub(crate) index: &'read MissionIndex<'source>,
    pub(crate) cancel: Option<&'read CancelToken>,
}

#[derive(Clone, Copy)]
enum Kind {
    Title,
    Description,
}

/// Projects each field against its own selected authored text and each language lookup.
pub(crate) fn titles<'read, 'source>(
    occurrence: ContractOccurrence,
    layers: &Layers<'read, 'source>,
    dependencies: &Dependencies<'read, 'source>,
    inputs: &Inputs<'read, '_, 'source>,
) -> Result<MissionTitles<'source>, CatalogError> {
    let mut resolve = |token: &str, variant: Option<&str>, evidence: &mut Consulted<'read>| {
        resolve_token(layers, dependencies, inputs, token, variant, evidence)
    };
    let origins = inputs.projection.origins();
    let title = select(layers, dependencies, origins, Kind::Title)?;
    let description = select(layers, dependencies, origins, Kind::Description)?;
    Ok(MissionTitles {
        contract: occurrence,
        title: mission_title_origin::project(
            inputs.projection,
            &title,
            &mut resolve,
            inputs.cancel,
        )?,
        description: mission_title_origin::project(
            inputs.projection,
            &description,
            &mut resolve,
            inputs.cancel,
        )?,
    })
}

fn select<'read, 'source>(
    layers: &Layers<'read, 'source>,
    dependencies: &Dependencies<'read, 'source>,
    origins: &CatalogOrigins,
    kind: Kind,
) -> Result<SelectedMissionText<'read, 'source>, CatalogError> {
    let mut selected = SelectedMissionText {
        key: None,
        decisions: Vec::new(),
        unreadable: false,
    };
    for layer in layers.iter() {
        let text = match kind {
            Kind::Title => &layer.title,
            Kind::Description => &layer.description,
        };
        record(&mut selected, &layer.parameters, origins)?;
        if consider(&mut selected, text, origins)? {
            return Ok(selected);
        }
    }
    if matches!(kind, Kind::Title) {
        record(&mut selected, &dependencies.links.template, origins)?;
        if let Some(template) = dependencies.template
            && consider(&mut selected, &template.title, origins)?
        {
            return Ok(selected);
        }
    }
    record(&mut selected, &dependencies.links.broker, origins)?;
    if let Some(broker) = dependencies.broker {
        let text = match kind {
            Kind::Title => &broker.title,
            Kind::Description => &broker.description,
        };
        consider(&mut selected, text, origins)?;
    }
    Ok(selected)
}

fn record(
    selected: &mut SelectedMissionText<'_, '_>,
    evidence: &Decisions,
    origins: &CatalogOrigins,
) -> Result<(), CatalogError> {
    evidence.retain(origins, &mut selected.decisions)?;
    selected.unreadable |= evidence.unreadable();
    Ok(())
}

fn consider<'read, 'source>(
    selected: &mut SelectedMissionText<'read, 'source>,
    candidate: &'read TextSelection<'source>,
    origins: &CatalogOrigins,
) -> Result<bool, CatalogError> {
    record(selected, &candidate.decisions, origins)?;
    selected.key = candidate.key.as_ref();
    Ok(selected.key.is_some())
}

fn resolve_token<'read, 'source>(
    layers: &Layers<'read, 'source>,
    dependencies: &Dependencies<'read, 'source>,
    inputs: &Inputs<'read, '_, 'source>,
    token: &str,
    variant: Option<&str>,
    evidence: &mut Consulted<'read>,
) -> Result<Option<ResolvedToken<'read, 'source>>, CatalogError> {
    let origins = inputs.projection.origins();
    for layer in layers.iter() {
        mission_title::check_cancel(inputs.cancel)?;
        evidence.all(&layer.parameters, origins)?;
        if let Some(choice) = layer.tokens.select(token, origins, evidence)? {
            return resolve_choice(choice, variant, inputs, evidence);
        }
    }
    evidence.all(&dependencies.links.template, origins)?;
    let Some(template) = dependencies.template else {
        return Ok(None);
    };
    let Some(choice) = template.tokens.select(token, origins, evidence)? else {
        return Ok(None);
    };
    resolve_choice(choice, variant, inputs, evidence)
}

fn resolve_choice<'read, 'source>(
    choice: &'read TokenChoice<'source>,
    variant: Option<&str>,
    inputs: &Inputs<'read, '_, 'source>,
    evidence: &mut Consulted<'read>,
) -> Result<Option<ResolvedToken<'read, 'source>>, CatalogError> {
    Ok(match choice {
        TokenChoice::Key(key) => Some(ResolvedToken::Key(key)),
        TokenChoice::MissionItem(choice) => Some(resolve_item(choice, variant, inputs, evidence)?),
        TokenChoice::Runtime(label) => Some(ResolvedToken::Runtime(label)),
        TokenChoice::Organization(guid) => inputs
            .index
            .organization(*guid)
            .map(|source| source.select(variant, inputs.projection.origins(), evidence))
            .transpose()?
            .flatten()
            .map(ResolvedToken::Key),
        TokenChoice::Unknown => None,
    })
}

fn resolve_item<'read, 'source>(
    choice: &MissionItemChoice,
    variant: Option<&str>,
    inputs: &Inputs<'read, '_, 'source>,
    evidence: &mut Consulted<'read>,
) -> Result<ResolvedToken<'read, 'source>, CatalogError> {
    let (guid, record_index) = match *choice {
        MissionItemChoice::Unresolved(reason) => return Ok(ResolvedToken::ItemGap(reason)),
        MissionItemChoice::Static { guid, record_index } => (guid, record_index),
    };
    let Some(item) = inputs.index.item(guid, record_index) else {
        return Ok(ResolvedToken::ItemGap(MissionItemGap::MissingItem));
    };
    Ok(
        match item.select(variant, inputs.projection.origins(), evidence)? {
            Ok(key) => ResolvedToken::ItemKey(SelectedItemKey {
                key,
                guid,
                record_index,
            }),
            Err(reason) => ResolvedToken::ItemGap(reason),
        },
    )
}
