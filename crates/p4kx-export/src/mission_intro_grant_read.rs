// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Guid, SourceUseRole, Value};

use crate::catalog_read::original::OriginalValuePart;
use crate::guid_text::CatalogGuid;
use crate::mission_layers;
use crate::{CatalogError, ExportAccountingError};

use super::Grants;
use super::read::{self, Evidence, Fact, Reader};

const COMPLETION_TAGS: &str = "ContractResult_CompletionTags";

pub(super) fn read<'fields, 'scope, 'read, 'source>(
    grants: &mut Grants,
    reader: &Reader<'fields, 'scope, 'read, 'source>,
    candidate: Fact<'fields, 'scope, 'read, 'source>,
    id: Option<Fact<'fields, 'scope, 'read, 'source>>,
    admitted: Guid,
    mut candidate_evidence: Evidence,
) -> Result<(), CatalogError> {
    let id = id.ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    if !matches!(id.value(), Value::Guid(actual) if *actual == admitted) {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    }
    let mut result_path = Evidence::default();
    let results = results(reader, candidate, &mut result_path)?;
    grants.survey(&result_path)?;
    let Some(results) = results else {
        return Ok(());
    };
    candidate_evidence.extend(&result_path)?;
    let identity = IntroId {
        value: CatalogGuid(admitted),
        fact: id,
    };
    for value in mission_layers::items_value(results.value()) {
        let mut branch = Evidence::default();
        let result = reader.fact(value)?;
        let completion = completion(reader, result, &mut branch)?;
        grants.survey(&branch)?;
        if let Some(completion) = completion {
            branch.extend(&candidate_evidence)?;
            record_matches(grants, reader, completion, &identity, &branch)?;
        }
    }
    Ok(())
}

fn results<'fields, 'scope, 'read, 'source>(
    reader: &Reader<'fields, 'scope, 'read, 'source>,
    candidate: Fact<'fields, 'scope, 'read, 'source>,
    evidence: &mut Evidence,
) -> Result<Option<Fact<'fields, 'scope, 'read, 'source>>, CatalogError> {
    let Some(block) = reader.field(Some(candidate), "contractResults", evidence)? else {
        return Ok(None);
    };
    let Some(block) = reader.first_struct(block, evidence)? else {
        return Ok(None);
    };
    reader.field(Some(block), "contractResults", evidence)
}

fn completion<'fields, 'scope, 'read, 'source>(
    reader: &Reader<'fields, 'scope, 'read, 'source>,
    result: Fact<'fields, 'scope, 'read, 'source>,
    evidence: &mut Evidence,
) -> Result<Option<Fact<'fields, 'scope, 'read, 'source>>, CatalogError> {
    evidence.consult(result)?;
    let Value::Struct(structure) = result.value() else {
        return Ok(None);
    };
    evidence.retain(result.observe(OriginalValuePart::ConcreteType, SourceUseRole::Selector)?)?;
    if structure.type_name() != COMPLETION_TAGS
        || !reader.first_mission_outcome(result, evidence)?
    {
        return Ok(None);
    }
    reader.field(Some(result), "completionTags", evidence)
}

struct IntroId<'fields, 'scope, 'read, 'source> {
    value: CatalogGuid,
    fact: Fact<'fields, 'scope, 'read, 'source>,
}

fn record_matches<'fields, 'scope, 'read, 'source>(
    grants: &mut Grants,
    reader: &Reader<'fields, 'scope, 'read, 'source>,
    completion: Fact<'fields, 'scope, 'read, 'source>,
    id: &IntroId<'fields, 'scope, 'read, 'source>,
    branch: &Evidence,
) -> Result<(), CatalogError> {
    for value in mission_layers::items_value(completion.value()) {
        let mut evidence = Evidence::default();
        let member = reader.fact(value)?;
        evidence.consult(member)?;
        let tag = if matches!(member.value(), Value::Struct(_))
            && let Some(field) = reader.declaration(Some(member), "tag", &mut evidence)?
        {
            read::tag(field, &mut evidence)?
        } else {
            None
        };
        grants.survey(&evidence)?;
        if let Some(tag) = tag {
            evidence.extend(branch)?;
            evidence.retain(
                id.fact
                    .observe(OriginalValuePart::Value, SourceUseRole::Payload)?,
            )?;
            grants.add(tag.guid, id.value, evidence)?;
        }
    }
    Ok(())
}
