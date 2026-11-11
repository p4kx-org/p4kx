// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Guid, SourceUse, StructValue};

use crate::catalog_origins::CatalogOrigins;
use crate::catalog_read::original::OriginalFlowFields;
use crate::contract_facts::{LayeredFacts, Stated};
use crate::mission_index::MissionIndex;
use crate::{CatalogError, ExportAccountingError};

use super::choice::{Choice, Selected};
use super::source::{Field, Layer, ParentLayers, Reader, TemplateCaller};

const SHARE: Field<bool> = Field::flag("canBeShared");
const ILLEGAL: Field<bool> = Field::flag("illegal");
const ONCE: Field<bool> = Field::flag("onceOnly");
const PLAYERS: Field<i64> = Field::integer("maxPlayersPerInstance");
const PRISON: Field<bool> = Field::flag("availableInPrison");
const ABANDON: Field<bool> = Field::flag("canReacceptAfterAbandoning");
const FAIL: Field<bool> = Field::flag("canReacceptAfterFailing");
const HIDE: Field<bool> = Field::flag("hideInMobiGlas");

/// The reusable parent choices and the actual source scope for sub-contract overrides.
pub(crate) struct PreparedFacts<'fields, 'scope, 'read, 'source> {
    reader: Reader<'fields, 'scope, 'read, 'source>,
    parent: Choices,
}

struct Choices {
    share: Choice<bool>,
    illegal: Choice<bool>,
    once: Choice<bool>,
    players: Choice<i64>,
    prison: Choice<bool>,
    abandon: Choice<bool>,
    fail: Choice<bool>,
    hide: Choice<bool>,
}

impl<'fields, 'scope, 'read, 'source> PreparedFacts<'fields, 'scope, 'read, 'source> {
    pub(crate) fn read(
        fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
        (handler, contract): (&StructValue<'source>, &StructValue<'source>),
        (index, origins): (&MissionIndex<'source>, &CatalogOrigins),
    ) -> Result<Self, CatalogError> {
        fields.check_context(&origins.uses, &origins.dataforge, fields.record_index())?;
        index.check_template_source(origins)?;
        let reader = Reader::new(fields);
        let mut layers = ParentLayers::new(handler, contract);
        let mut template = TemplateDefaults {
            index,
            contract,
            origins,
            caller: None,
        };
        let parent = Choices {
            share: template.choice(&reader, &mut layers, TemplateField::Sharing)?,
            illegal: template.choice(&reader, &mut layers, TemplateField::Legality)?,
            once: reader.parent(&ONCE, &mut layers)?,
            players: reader.parent(&PLAYERS, &mut layers)?,
            prison: reader.parent(&PRISON, &mut layers)?,
            abandon: reader.parent(&ABANDON, &mut layers)?,
            fail: reader.parent(&FAIL, &mut layers)?,
            hide: reader.parent(&HIDE, &mut layers)?,
        };
        Ok(Self { reader, parent })
    }

    pub(crate) fn resolve(
        &self,
        sub: Option<&StructValue<'source>>,
    ) -> Result<LayeredFacts, CatalogError> {
        let layer = sub.map(|sub| self.reader.sub(sub)).transpose()?;
        self.parent.project(&self.reader, layer.as_ref())
    }
}

impl Choices {
    fn project<'fields, 'scope, 'read, 'source>(
        &self,
        reader: &Reader<'fields, 'scope, 'read, 'source>,
        child: Option<&Layer<'fields, 'scope, 'read, 'source>>,
    ) -> Result<LayeredFacts, CatalogError> {
        let flag = |field, parent| bind(reader, child, field, parent);
        Ok(LayeredFacts {
            can_be_shared: flag(&SHARE, &self.share)?,
            illegal: flag(&ILLEGAL, &self.illegal)?,
            once_only: flag(&ONCE, &self.once)?,
            max_players_per_instance: bind(reader, child, &PLAYERS, &self.players)?,
            available_in_prison: flag(&PRISON, &self.prison)?,
            can_reaccept_after_abandoning: flag(&ABANDON, &self.abandon)?,
            can_reaccept_after_failing: flag(&FAIL, &self.fail)?,
            hide_in_mobi_glas: flag(&HIDE, &self.hide)?,
        })
    }
}

fn bind<'fields, 'scope, 'read, 'source, T: Copy>(
    reader: &Reader<'fields, 'scope, 'read, 'source>,
    child: Option<&Layer<'fields, 'scope, 'read, 'source>>,
    field: &Field<T>,
    parent: &Choice<T>,
) -> Result<Stated<T>, CatalogError> {
    let sources = reader.sources();
    if let Some(child) = child {
        reader.child(field, child, parent)?.bind(sources)
    } else {
        parent.bind(sources)
    }
}

#[derive(Clone, Copy)]
enum TemplateField {
    Sharing,
    Legality,
}

impl TemplateField {
    const fn parameter(self) -> &'static Field<bool> {
        match self {
            Self::Sharing => &SHARE,
            Self::Legality => &ILLEGAL,
        }
    }
}

struct TemplateDefaults<'owner, 'source> {
    index: &'owner MissionIndex<'source>,
    contract: &'owner StructValue<'source>,
    origins: &'owner CatalogOrigins,
    caller: Option<TemplateCaller>,
}

impl<'source> TemplateDefaults<'_, 'source> {
    fn choice<'fields, 'scope, 'read>(
        &mut self,
        reader: &Reader<'fields, 'scope, 'read, 'source>,
        layers: &mut ParentLayers<'_, 'fields, 'scope, 'read, 'source>,
        field: TemplateField,
    ) -> Result<Choice<bool>, CatalogError> {
        reader.with_template(field.parameter(), layers, |evidence| {
            let caller = match &mut self.caller {
                Some(caller) => caller,
                empty => empty.insert(reader.template_caller(self.contract)?),
            };
            evidence.extend(&caller.evidence)?;
            let Some(guid) = caller.guid else {
                return Ok(None);
            };
            let mut consulted = Vec::new();
            let selected = self.retain(guid, field, &mut consulted)?;
            for source in consulted {
                evidence.retain(source)?;
            }
            selected
                .map(|(value, payload)| Selected::template(value, payload))
                .transpose()
        })
    }

    fn retain(
        &self,
        guid: Guid,
        field: TemplateField,
        consulted: &mut Vec<SourceUse>,
    ) -> Result<Option<(bool, SourceUse)>, CatalogError> {
        let Some(template) = self.index.template(guid) else {
            for failure in self.index.failed_templates(guid) {
                failure.retain_attempted(self.origins, consulted)?;
            }
            return Ok(None);
        };
        let facts = template
            .facts
            .as_deref()
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        match field {
            TemplateField::Sharing => facts.retain_can_be_shared(self.origins, consulted),
            TemplateField::Legality => facts.retain_illegal(self.origins, consulted),
        }
    }
}
