// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUseRole, SourceValue, SourcedValue, StructValue, Value};

use crate::catalog_read::original::{OriginalFlowFields, OriginalValuePart};
use crate::contract_missions::StandingLevel;
use crate::guid_text::CatalogGuid;
use crate::mission_layers;
use crate::{CatalogError, ExportAccountingError};

use super::AuthoredIntro;
use super::read::{self, Evidence, Fact, Reader, Tag};

const COMPLETED_TAGS: &str = "ContractPrerequisite_CompletedContractTags";
const ADDITIONAL: &str = "additionalPrerequisites";

/// Completion requirements captured before the cross-Record grant join.
#[derive(Clone, Debug, Default)]
pub(crate) struct Requirements {
    items: Vec<AuthoredIntro>,
    evidence: Evidence,
}

impl Requirements {
    pub(crate) fn shared<'source>(
        fields: &OriginalFlowFields<'_, '_, 'source>,
        handler: &StructValue<'source>,
        contract: &StructValue<'source>,
    ) -> Result<Self, CatalogError> {
        let reader = Reader::new(fields);
        let mut requirements = Self::default();
        let mut inherited = Evidence::default();
        let handler = reader.owner(handler)?;
        let availability = reader.field(Some(handler), "defaultAvailability", &mut inherited)?;
        let availability = availability
            .map(|field| reader.first_struct(field, &mut inherited))
            .transpose()?
            .flatten();
        if let Some(availability) = availability {
            requirements.layer(
                &reader,
                availability,
                "prerequisites",
                StandingLevel::Handler,
                inherited,
            )?;
        } else {
            requirements.evidence.extend(&inherited)?;
        }
        requirements.layer(
            &reader,
            reader.owner(contract)?,
            ADDITIONAL,
            StandingLevel::Contract,
            Evidence::default(),
        )?;
        Ok(requirements)
    }

    pub(crate) fn row<'source>(
        &self,
        fields: &OriginalFlowFields<'_, '_, 'source>,
        sub: Option<&StructValue<'source>>,
    ) -> Result<SourcedValue<Vec<AuthoredIntro>>, CatalogError> {
        let reader = Reader::new(fields);
        let mut requirements = self.clone();
        if let Some(sub) = sub {
            requirements.layer(
                &reader,
                reader.owner(sub)?,
                ADDITIONAL,
                StandingLevel::SubContract,
                Evidence::default(),
            )?;
        }
        Ok(requirements
            .evidence
            .bind(reader.sources(), requirements.items))
    }

    fn layer<'fields, 'scope, 'read, 'source>(
        &mut self,
        reader: &Reader<'fields, 'scope, 'read, 'source>,
        owner: Fact<'fields, 'scope, 'read, 'source>,
        name: &str,
        level: StandingLevel,
        mut path: Evidence,
    ) -> Result<(), CatalogError> {
        let field = reader.field(Some(owner), name, &mut path)?;
        self.evidence.extend(&path)?;
        if let Some(field) = field {
            for member in mission_layers::items_value(field.value()) {
                let prerequisite = reader.fact(member)?;
                self.prerequisite(reader, prerequisite, level, &path)?;
            }
        }
        Ok(())
    }

    fn prerequisite<'fields, 'scope, 'read, 'source>(
        &mut self,
        reader: &Reader<'fields, 'scope, 'read, 'source>,
        prerequisite: Fact<'fields, 'scope, 'read, 'source>,
        level: StandingLevel,
        path: &Evidence,
    ) -> Result<(), CatalogError> {
        let mut observed = Evidence::default();
        let tags = tags(reader, prerequisite, &mut observed)?;
        self.evidence.extend(&observed)?;
        let Some(tags) = tags else {
            return Ok(());
        };
        observed.extend(path)?;
        for item in mission_layers::items_value(tags.value()) {
            let selected = reader.fact(item)?;
            let mut slot = Evidence::default();
            let tag = read::tag(selected, &mut slot)?;
            self.evidence.extend(&slot)?;
            if let Some(tag) = tag {
                slot.extend(&observed)?;
                let requirement = authored(reader, tag, level, slot)?;
                self.items
                    .try_reserve(1)
                    .map_err(CatalogError::SourceUseAllocation)?;
                self.items.push(requirement);
            }
        }
        Ok(())
    }
}

fn tags<'fields, 'scope, 'read, 'source>(
    reader: &Reader<'fields, 'scope, 'read, 'source>,
    prerequisite: Fact<'fields, 'scope, 'read, 'source>,
    evidence: &mut Evidence,
) -> Result<Option<Fact<'fields, 'scope, 'read, 'source>>, CatalogError> {
    evidence.consult(prerequisite)?;
    let Value::Struct(structure) = prerequisite.value() else {
        return Ok(None);
    };
    evidence
        .retain(prerequisite.observe(OriginalValuePart::ConcreteType, SourceUseRole::Selector)?)?;
    if structure.type_name() != COMPLETED_TAGS {
        return Ok(None);
    }
    let Some(list) = reader.field(
        Some(prerequisite),
        "requiredCompletedContractTags",
        evidence,
    )?
    else {
        return Ok(None);
    };
    let Some(list) = reader.first_struct(list, evidence)? else {
        return Ok(None);
    };
    reader.field(Some(list), "tags", evidence)
}

fn authored(
    reader: &Reader<'_, '_, '_, '_>,
    tag: Tag<'_, '_, '_, '_>,
    level: StandingLevel,
    mut evidence: Evidence,
) -> Result<AuthoredIntro, CatalogError> {
    let level = evidence.bind(reader.sources(), level);
    let datum = tag.observed.guid()?;
    let (uses, complete) = datum.contributions();
    if datum.value() != &SourceValue::Present(tag.guid) {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    }
    evidence.extend_uses(uses)?;
    if !complete {
        evidence.incomplete();
    }
    let tag_guid = evidence.bind(reader.sources(), CatalogGuid(tag.guid));
    Ok(AuthoredIntro {
        level,
        tag_guid,
        lookup: tag.lookup(reader.sources())?,
    })
}
