// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{ENTITY_TYPE, Entities};
use p4kx_extraction::{
    Guid, Record, SourceLocation, SourceScope, SourceUse, SourceUseRole, SourceUses, SourceValue,
    SourcedValue,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::player_source::PlayerSourceError;

pub(crate) type Classes<'forge, 'scope> = Option<(&'scope Entities<'forge>, &'scope SourceScope)>;

#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
pub(crate) enum ClassTarget {
    Empty,
    Unmatched,
    Matched { record: ClassRecord },
    UnexpectedRecordType { record: ClassRecord },
    Ambiguous { records: Vec<ClassRecord> },
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ClassRecord {
    record_index: u32,
    guid: SourcedValue<String>,
    record_type: SourcedValue<String>,
}

pub(crate) fn by_guid(
    sources: &SourceUses,
    classes: Classes<'_, '_>,
    selector: &SourceUse,
    guid: Guid,
) -> Result<SourcedValue<ClassTarget>, PlayerSourceError> {
    if guid == Guid::default() {
        return Ok(empty(sources, selector));
    }
    project(sources, classes, selector, |entities| {
        entities.forge().records_by_guid(guid).collect()
    })
}

pub(crate) fn by_name(
    sources: &SourceUses,
    classes: Classes<'_, '_>,
    selector: &SourceUse,
    class: &str,
) -> Result<SourcedValue<ClassTarget>, PlayerSourceError> {
    if class.is_empty() {
        return Ok(empty(sources, selector));
    }
    project(sources, classes, selector, |entities| {
        entities.named_class_candidates(class).to_vec()
    })
}

fn empty(sources: &SourceUses, selector: &SourceUse) -> SourcedValue<ClassTarget> {
    sources.value(
        SourceValue::Present(ClassTarget::Empty),
        std::slice::from_ref(selector),
        true,
    )
}

fn project<'forge>(
    sources: &SourceUses,
    classes: Classes<'forge, '_>,
    selector: &SourceUse,
    find: impl FnOnce(&Entities<'forge>) -> Vec<Record<'forge>>,
) -> Result<SourcedValue<ClassTarget>, PlayerSourceError> {
    let Some((entities, scope)) = classes else {
        return Ok(sources.value(
            SourceValue::Unreadable,
            std::slice::from_ref(selector),
            false,
        ));
    };
    let mut uses = vec![selector.clone()];
    let matches = find(entities);
    let complete = matches.iter().all(|record| record.type_name().is_ok());
    let selected = match matches.as_slice() {
        [] => {
            uses.push(scope.at(
                SourceLocation::Metadata {
                    field: "entityClassIndex".into(),
                },
                SourceUseRole::Payload,
            )?);
            ClassTarget::Unmatched
        }
        [record] => {
            let projected = ClassRecord::read(sources, scope, record, &mut uses)?;
            if matches!(record.type_name(), Ok(ENTITY_TYPE)) {
                ClassTarget::Matched { record: projected }
            } else {
                ClassTarget::UnexpectedRecordType { record: projected }
            }
        }
        records => ClassTarget::Ambiguous {
            records: records
                .iter()
                .map(|record| ClassRecord::read(sources, scope, record, &mut uses))
                .collect::<Result<_, _>>()?,
        },
    };
    Ok(sources.value(SourceValue::Present(selected), &uses, complete))
}

impl ClassRecord {
    fn read(
        sources: &SourceUses,
        scope: &SourceScope,
        record: &Record<'_>,
        uses: &mut Vec<SourceUse>,
    ) -> Result<Self, PlayerSourceError> {
        let record_index = record.index();
        let field = |field: &str| {
            scope.at(
                SourceLocation::DataForge {
                    record_index,
                    field: field.to_owned(),
                },
                SourceUseRole::Payload,
            )
        };
        let guid = field("$guid")?;
        let kind = field("$type")?;
        uses.extend([guid.clone(), kind.clone()]);
        let record_type = record.type_name();
        let complete = record_type.is_ok();
        let record_type = record_type.map_or(SourceValue::Unreadable, |name| {
            SourceValue::Present(name.to_owned())
        });
        Ok(Self {
            record_index,
            guid: sources.value(
                SourceValue::Present(record.guid().to_string()),
                &[guid],
                true,
            ),
            record_type: sources.value(record_type, &[kind], complete),
        })
    }
}
