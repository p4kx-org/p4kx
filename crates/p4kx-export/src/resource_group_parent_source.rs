// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    CancelToken, Guid, Record, RecordTree, SourceUse, SourceUseRole, Value, ValueError,
};

use crate::catalog_origins::CatalogOrigins;
use crate::catalog_read::CatalogRecordContext;
use crate::guid_text::CatalogGuid;
use crate::record_header_field::{RecordHeaderField, RecordHeaderInput};
use crate::{CatalogError, Kind};

/// Original target metadata retained at the selected group's canonical read.
#[derive(Debug)]
pub(crate) struct ParentTarget {
    pub(crate) guid: CatalogGuid,
    pub(crate) record_index: u32,
    pub(crate) record_path: String,
    pub(crate) source_type: String,
}

#[derive(Debug)]
struct Slot {
    value: SourceUse,
    self_guid: Option<SourceUse>,
    target: Option<Guid>,
    complete: bool,
}

/// Actual selection inputs of one required group, without unrelated later contributions.
#[derive(Debug)]
pub(crate) struct ParentSource {
    pub(super) target: Option<ParentTarget>,
    inputs: Vec<SourceUse>,
    slots: Vec<Slot>,
    complete: bool,
}

impl ParentSource {
    pub(crate) fn read(
        tree: &RecordTree<'_>,
        origins: &CatalogOrigins,
        cancel: Option<&CancelToken>,
    ) -> Result<(Self, Vec<(usize, Guid)>), CatalogError> {
        let context = origins.record(tree.record_index());
        let mut source = Self {
            target: Some(ParentTarget {
                guid: CatalogGuid(tree.guid()),
                record_index: tree.record_index(),
                record_path: tree.record_path().to_owned(),
                source_type: tree.type_name().to_owned(),
            }),
            inputs: headers(&context)?,
            slots: Vec::new(),
            complete: true,
        };
        let value = tree.field("groups");
        let (role, complete) = observation(value);
        source.inputs.push(context.observe("groups", role)?);
        source.complete = complete;
        let children = if let Some(Value::Array(values)) = value {
            source.read_slots(values, &context, cancel)?
        } else {
            Vec::new()
        };
        Ok((source, children))
    }

    fn read_slots(
        &mut self,
        values: &[Value<'_>],
        context: &CatalogRecordContext<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<Vec<(usize, Guid)>, CatalogError> {
        let mut children = Vec::new();
        self.slots.reserve(values.len());
        for (ordinal, value) in values.iter().enumerate() {
            if cancel.is_some_and(CancelToken::is_cancelled) {
                return Err(CatalogError::Cancelled);
            }
            let (slot, target) = slot(value, ordinal, context)?;
            self.slots.push(slot);
            if let Some(target) = target {
                children.push((ordinal, target));
            }
        }
        Ok(children)
    }

    pub(crate) fn canonical_failure(
        record: &Record<'_>,
        origins: &CatalogOrigins,
    ) -> Result<Self, CatalogError> {
        let context = origins.record(record.index());
        let mut inputs = headers(&context)?;
        inputs.push(context.observe("", SourceUseRole::Attempted)?);
        Ok(Self {
            target: None,
            inputs,
            slots: Vec::new(),
            complete: false,
        })
    }

    pub(crate) fn metadata_failure(
        record: &Record<'_>,
        origins: &CatalogOrigins,
    ) -> Result<Option<Self>, CatalogError> {
        let path = record.record_path();
        if path
            .as_ref()
            .is_ok_and(|path| !Kind::ResourceGroups.records().matches_path(path))
        {
            return Ok(None);
        }
        let kind = record.type_name();
        if kind
            .as_ref()
            .is_ok_and(|name| *name != Kind::ResourceGroups.records().type_name)
        {
            return Ok(None);
        }
        let context = origins.record(record.index());
        let mut inputs =
            vec![context.observe_header(RecordHeaderField::RecordIndex, SourceUseRole::Selector)?];
        for (field, readable) in [
            (RecordHeaderInput::NormalizedPath, path.is_ok()),
            (
                RecordHeaderInput::Field(RecordHeaderField::SourceType),
                kind.is_ok(),
            ),
            (
                RecordHeaderInput::Field(RecordHeaderField::RecordName),
                record.name().is_ok(),
            ),
        ] {
            context.append_header_input(
                field,
                if readable {
                    SourceUseRole::Selector
                } else {
                    SourceUseRole::Attempted
                },
                &mut inputs,
            )?;
            if !readable {
                break;
            }
        }
        Ok(Some(Self {
            target: None,
            inputs,
            slots: Vec::new(),
            complete: false,
        }))
    }

    pub(super) fn append(
        &self,
        child: Guid,
        through: Option<usize>,
        uses: &mut Vec<SourceUse>,
        cancel: Option<&CancelToken>,
    ) -> Result<bool, CatalogError> {
        uses.extend(self.inputs.iter().cloned());
        let length = through.map_or(self.slots.len(), |position| position.saturating_add(1));
        let mut complete = self.complete;
        for slot in self.slots.iter().take(length) {
            if cancel.is_some_and(CancelToken::is_cancelled) {
                return Err(CatalogError::Cancelled);
            }
            uses.push(slot.value.clone());
            if slot.target == Some(child) {
                uses.extend(slot.self_guid.iter().cloned());
            }
            complete &= slot.complete;
        }
        Ok(complete)
    }
}

fn headers(context: &CatalogRecordContext<'_>) -> Result<Vec<SourceUse>, CatalogError> {
    let mut uses = Vec::new();
    for field in [
        RecordHeaderInput::NormalizedPath,
        RecordHeaderInput::Field(RecordHeaderField::SourceType),
        RecordHeaderInput::Field(RecordHeaderField::RecordName),
        RecordHeaderInput::Field(RecordHeaderField::RecordIndex),
    ] {
        context.append_header_input(field, SourceUseRole::Selector, &mut uses)?;
    }
    Ok(uses)
}

const fn observation(value: Option<&Value<'_>>) -> (SourceUseRole, bool) {
    if matches!(value, Some(Value::Error(error)) if !matches!(error, ValueError::DanglingReference { .. } | ValueError::UnexpectedReferenceType { .. }))
    {
        (SourceUseRole::Attempted, false)
    } else {
        (SourceUseRole::Selector, true)
    }
}

fn slot(
    value: &Value<'_>,
    ordinal: usize,
    context: &CatalogRecordContext<'_>,
) -> Result<(Slot, Option<Guid>), CatalogError> {
    let (role, complete) = observation(Some(value));
    let input = context.observe(&format!("groups[{ordinal}]"), role)?;
    let (target, self_guid) = if let Value::Reference(target) = value
        && target.location().is_none()
    {
        (
            Some(target.guid()),
            Some(context.observe_header(RecordHeaderField::Guid, SourceUseRole::Selector)?),
        )
    } else {
        (None, None)
    };
    Ok((
        Slot {
            value: input,
            self_guid,
            target,
            complete,
        },
        target,
    ))
}
