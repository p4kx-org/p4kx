// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use crate::annotations::TAG_TYPE;
use crate::tags::{
    TAG_DATABASE_TYPE, TagBuildEvent, TagPathEdgeKind, TagPathIncoming, TagRelationIssue,
    TagTreeIssues,
};
use crate::{DataForge, Guid, Record, RecordTree, TreeBuilder, Value, ValueError};

use super::Ticker;

pub(super) struct Raw<'f> {
    pub(super) record: Record<'f>,
    pub(super) guid: Guid,
    pub(super) name: &'f str,
    pub(super) legacy_id: Option<u32>,
    pub(super) children: Vec<Child>,
}

#[derive(Clone, Copy)]
pub(super) struct Child {
    pub(super) target: Guid,
    pub(super) ordinal: usize,
}

pub(super) struct Root<'f> {
    pub(super) record: Record<'f>,
    pub(super) target: Guid,
    pub(super) ordinal: usize,
}

pub(super) struct ReadTags<'f> {
    pub(super) raws: Vec<Raw<'f>>,
    pub(super) roots: Vec<Root<'f>>,
}

pub(super) fn read<'f, E>(
    forge: &'f DataForge<'f>,
    issues: &mut TagTreeIssues,
    ticker: &mut Ticker<'_>,
    observe: &mut impl for<'r> FnMut(TagBuildEvent<'r, 'f>) -> Result<(), E>,
) -> Result<Option<ReadTags<'f>>, E> {
    let mut builder = TreeBuilder::new();
    let mut raws = Vec::new();
    let mut seen = HashSet::new();
    let mut roots = Vec::new();
    for record in forge.records() {
        if ticker.stopped() {
            return Ok(None);
        }
        let Some(tree) = canonical(&mut builder, &record, issues, observe)? else {
            continue;
        };
        if tree.type_name() == TAG_DATABASE_TYPE {
            let mut listed = Vec::new();
            references(
                tree.field("tags"),
                &record,
                TagPathEdgeKind::Root,
                &mut listed,
                issues,
                ticker,
                observe,
            )?;
            if ticker.stopped() {
                return Ok(None);
            }
            roots.extend(listed.into_iter().map(|child| Root {
                record,
                target: child.target,
                ordinal: child.ordinal,
            }));
            observe(TagBuildEvent::Projected { record: &record })?;
        } else {
            let tag = tag(&record, &tree, &mut seen, issues, ticker, observe)?;
            if ticker.stopped() {
                return Ok(None);
            }
            raws.extend(tag);
        }
    }
    Ok(Some(ReadTags { raws, roots }))
}

fn canonical<'f, E>(
    builder: &mut TreeBuilder<'f>,
    record: &Record<'f>,
    issues: &mut TagTreeIssues,
    observe: &mut impl for<'r> FnMut(TagBuildEvent<'r, 'f>) -> Result<(), E>,
) -> Result<Option<RecordTree<'f>>, E> {
    let type_name = match record.type_name() {
        Ok(name) => name,
        Err(error) => {
            issues.unclassified += 1;
            observe(TagBuildEvent::MetadataFailed {
                record,
                error: &error,
            })?;
            return Ok(None);
        }
    };
    if type_name != TAG_TYPE && type_name != TAG_DATABASE_TYPE {
        return Ok(None);
    }
    let tree = builder.build(record);
    observe(TagBuildEvent::CanonicalRead {
        record,
        outcome: tree.as_ref(),
    })?;
    if let Ok(tree) = tree {
        Ok(Some(tree))
    } else {
        issues.unreadable += 1;
        Ok(None)
    }
}

fn tag<'f, E>(
    record: &Record<'f>,
    tree: &RecordTree<'f>,
    seen: &mut HashSet<Guid>,
    issues: &mut TagTreeIssues,
    ticker: &mut Ticker<'_>,
    observe: &mut impl for<'r> FnMut(TagBuildEvent<'r, 'f>) -> Result<(), E>,
) -> Result<Option<Raw<'f>>, E> {
    if !seen.insert(record.guid()) {
        issues.duplicate += 1;
        observe(TagBuildEvent::DuplicateSkipped { record })?;
        return Ok(None);
    }
    let name = match tree.field("tagName") {
        Some(&Value::String(name)) => name,
        _ => "",
    };
    let legacy_id = match tree.field("legacyGUID") {
        Some(Value::UInt32(id)) if *id != u32::MAX => Some(*id),
        _ => None,
    };
    let mut children = Vec::new();
    references(
        tree.field("children"),
        record,
        TagPathEdgeKind::Child,
        &mut children,
        issues,
        ticker,
        observe,
    )?;
    Ok(Some(Raw {
        record: *record,
        guid: record.guid(),
        name,
        legacy_id,
        children,
    }))
}

fn references<'f, E>(
    value: Option<&Value<'_>>,
    record: &Record<'f>,
    kind: TagPathEdgeKind,
    out: &mut Vec<Child>,
    issues: &mut TagTreeIssues,
    ticker: &mut Ticker<'_>,
    observe: &mut impl for<'r> FnMut(TagBuildEvent<'r, 'f>) -> Result<(), E>,
) -> Result<(), E> {
    let Some(Value::Array(items)) = value else {
        return Ok(());
    };
    out.reserve(items.len());
    for (ordinal, item) in items.iter().enumerate() {
        if ticker.stopped() {
            return Ok(());
        }
        let (target, reason) = match item {
            Value::Reference(reference) => {
                out.push(Child {
                    target: reference.guid(),
                    ordinal,
                });
                continue;
            }
            Value::Null => continue,
            Value::Error(ValueError::DanglingReference { guid }) => {
                (Some(*guid), TagRelationIssue::Dangling)
            }
            _ => (None, TagRelationIssue::MalformedValue),
        };
        issues.dangling += 1;
        observe(TagBuildEvent::RelationSkipped {
            incoming: TagPathIncoming::new(record, kind, ordinal),
            target,
            reason,
        })?;
    }
    Ok(())
}
