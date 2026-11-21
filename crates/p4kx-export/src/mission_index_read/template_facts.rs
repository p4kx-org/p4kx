// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{RecordTree, SourceLocation, SourceUse, SourceUseRole, Value};

use crate::CatalogError;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValueFact, OriginalValuePart};
use crate::mission_layers::items_value;

use super::template_read::TemplateReadSource;

/// Original Template default decisions retained by the mission dependency read.
#[derive(Debug)]
pub(crate) struct TemplateFacts {
    source: TemplateReadSource,
    can_be_shared: DeferredBool,
    illegal: DeferredBool,
}

impl TemplateFacts {
    pub(crate) fn read<'source>(
        tree: &RecordTree<'source>,
        fields: &OriginalFlowFields<'_, '_, 'source>,
        origins: &CatalogOrigins,
    ) -> Result<Self, CatalogError> {
        fields.check_tree(tree)?;
        fields.check_context(&origins.uses, &origins.dataforge, tree.record_index())?;
        let reader = Reader { fields };
        let mut can_be_shared = DeferredBool::default();
        can_be_shared.selected = reader.can_be_shared(&mut can_be_shared.consulted)?;
        let mut illegal = DeferredBool::default();
        illegal.selected = reader.illegal(&mut illegal.consulted)?;
        Ok(Self {
            source: TemplateReadSource::new(origins),
            can_be_shared,
            illegal,
        })
    }

    pub(crate) fn retain_can_be_shared(
        &self,
        origins: &CatalogOrigins,
        decisions: &mut Vec<SourceUse>,
    ) -> Result<Option<(bool, SourceUse)>, CatalogError> {
        self.retain(&self.can_be_shared, origins, decisions)
    }

    pub(crate) fn retain_illegal(
        &self,
        origins: &CatalogOrigins,
        decisions: &mut Vec<SourceUse>,
    ) -> Result<Option<(bool, SourceUse)>, CatalogError> {
        self.retain(&self.illegal, origins, decisions)
    }

    fn retain(
        &self,
        fact: &DeferredBool,
        origins: &CatalogOrigins,
        decisions: &mut Vec<SourceUse>,
    ) -> Result<Option<(bool, SourceUse)>, CatalogError> {
        self.source.check(origins)?;
        decisions
            .try_reserve(fact.consulted.0.len() + usize::from(fact.selected.is_some()))
            .map_err(CatalogError::SourceUseAllocation)?;
        for (location, role) in &fact.consulted.0 {
            decisions.push(self.source.observe(location.clone(), *role)?);
        }
        let Some((value, location)) = &fact.selected else {
            return Ok(None);
        };
        decisions.push(
            self.source
                .observe(location.clone(), SourceUseRole::Selector)?,
        );
        let payload = self
            .source
            .observe(location.clone(), SourceUseRole::Payload)?;
        Ok(Some((*value, payload)))
    }
}

#[derive(Debug, Default)]
struct DeferredBool {
    consulted: DeferredDecisions,
    selected: Option<(bool, SourceLocation)>,
}

#[derive(Debug, Default)]
struct DeferredDecisions(Vec<(SourceLocation, SourceUseRole)>);

impl DeferredDecisions {
    fn add(&mut self, location: SourceLocation, role: SourceUseRole) -> Result<(), CatalogError> {
        self.0
            .try_reserve(1)
            .map_err(CatalogError::SourceUseAllocation)?;
        self.0.push((location, role));
        Ok(())
    }

    fn consult(&mut self, fact: &OriginalValueFact<'_, '_, '_>) -> Result<(), CatalogError> {
        let role = if matches!(fact.value(), Value::Error(_)) {
            SourceUseRole::Attempted
        } else {
            SourceUseRole::Selector
        };
        self.add(fact.location(OriginalValuePart::Value), role)
    }
}

type Fact<'fields, 'scope, 'read, 'source> = &'fields OriginalValueFact<'scope, 'read, 'source>;

struct Reader<'fields, 'scope, 'read, 'source> {
    fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
}

impl<'fields, 'scope, 'read, 'source> Reader<'fields, 'scope, 'read, 'source> {
    fn illegal(
        &self,
        decisions: &mut DeferredDecisions,
    ) -> Result<Option<(bool, SourceLocation)>, CatalogError> {
        let Some(display) = self.first_struct("contractDisplayInfo", decisions)? else {
            return Ok(None);
        };
        self.boolean(display, "illegal", decisions)
    }

    fn can_be_shared(
        &self,
        decisions: &mut DeferredDecisions,
    ) -> Result<Option<(bool, SourceLocation)>, CatalogError> {
        let Some(class) = self.first_struct("contractClass", decisions)? else {
            return Ok(None);
        };
        let Some(params) = self.field(Some(class), "additionalParams", decisions)? else {
            return Ok(None);
        };
        decisions.consult(params)?;
        for value in items_value(params.value()) {
            let entry = self.fields.fact(value)?;
            if !std::ptr::eq(entry, params) {
                decisions.consult(entry)?;
            }
            if matches!(value, Value::Struct(_))
                && let Some(selected) = self.boolean(entry, "canBeShared", decisions)?
            {
                return Ok(Some(selected));
            }
        }
        Ok(None)
    }

    fn first_struct(
        &self,
        name: &str,
        decisions: &mut DeferredDecisions,
    ) -> Result<Option<Fact<'fields, 'scope, 'read, 'source>>, CatalogError> {
        let Some(container) = self.field(None, name, decisions)? else {
            return Ok(None);
        };
        decisions.consult(container)?;
        self.fields.first_struct_observed(container, |value| {
            decisions.consult(self.fields.fact(value)?)
        })
    }

    fn boolean(
        &self,
        owner: Fact<'fields, 'scope, 'read, 'source>,
        name: &str,
        decisions: &mut DeferredDecisions,
    ) -> Result<Option<(bool, SourceLocation)>, CatalogError> {
        let Some(fact) = self.field(Some(owner), name, decisions)? else {
            return Ok(None);
        };
        if let Value::Bool(value) = fact.value() {
            return Ok(Some((*value, fact.location(OriginalValuePart::Value))));
        }
        decisions.consult(fact)?;
        Ok(None)
    }

    fn field(
        &self,
        parent: Option<Fact<'fields, 'scope, 'read, 'source>>,
        name: &str,
        decisions: &mut DeferredDecisions,
    ) -> Result<Option<Fact<'fields, 'scope, 'read, 'source>>, CatalogError> {
        if let Some((_, fact)) = self.fields.field(parent, name)? {
            return Ok(Some(fact));
        }
        decisions.add(
            self.fields.survey_location(parent)?,
            SourceUseRole::Selector,
        )?;
        Ok(None)
    }
}
