// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use anyhow::Result;
use p4kx_assembly::{PrefabIssue, PrefabIssueKind, PrefabIssueSource, PrefabLinkSource};
use p4kx_extraction::FieldOrigins;
use p4kx_extraction::SourceUseRole::{Conversion, Payload};

use super::route;
use super::sites::{PrefabAttribute, Proof, Sites};

pub(super) fn collect(
    sites: &mut Sites<'_, '_>,
    fields: &mut FieldOrigins,
    index: usize,
    issue: &PrefabIssue,
    source: &PrefabIssueSource,
) -> Result<Proof> {
    let path = format!("/issues/{index}");
    let route = route::trace(sites, source.instance_route())?;
    let shared = sites
        .uses
        .shared_origin(&route.selection.uses, route.selection.complete);
    sites.put_shared(
        fields,
        &format!("{path}/container"),
        &shared,
        &route.container,
    )?;
    let mut name = Proof::default();
    if let Some(object) = source.object() {
        let source = object.source();
        sites.attribute(
            &mut name,
            object.prefab.file,
            source.map(|source| &source.transform.element),
            source.and_then(|source| source.name.as_ref()),
            "Name",
            Payload,
        )?;
    }
    sites.put_shared(fields, &format!("{path}/name"), &shared, &name)?;

    let mut decision = decision(sites, source, &issue.kind)?;
    sites.put_shared(fields, &format!("{path}/kind"), &shared, &decision)?;
    reason(sites, &mut decision, source, &issue.kind)?;
    sites.put_shared(fields, &format!("{path}/reason"), &shared, &decision)?;
    let mut aggregate = route.selection;
    aggregate.append(route.container);
    aggregate.append(name);
    aggregate.append(decision);
    sites.put(fields, &path, &aggregate)?;
    Ok(aggregate)
}

fn decision(
    sites: &mut Sites<'_, '_>,
    source: &PrefabIssueSource,
    kind: &PrefabIssueKind,
) -> Result<Proof> {
    let mut proof = Proof::default();
    match kind {
        PrefabIssueKind::ObjectsLeftOut { .. } | PrefabIssueKind::Attached { .. } => {
            sites.objects(&mut proof, source.prefab(), Conversion)?;
        }
        PrefabIssueKind::Missing { .. }
        | PrefabIssueKind::Cycle { .. }
        | PrefabIssueKind::TooDeep { .. }
        | PrefabIssueKind::LibraryUnreadable { .. }
        | PrefabIssueKind::GuidMismatch { .. } => {
            if let (Some(object), Some(link)) = (source.object(), source.link()) {
                route::link(sites, &mut proof, object, link)?;
            } else {
                proof.complete = false;
            }
        }
        _ => proof.complete = false,
    }
    Ok(proof)
}

fn reason(
    sites: &mut Sites<'_, '_>,
    proof: &mut Proof,
    source: &PrefabIssueSource,
    kind: &PrefabIssueKind,
) -> Result<()> {
    match kind {
        PrefabIssueKind::ObjectsLeftOut { .. } | PrefabIssueKind::Attached { .. } => {
            sites.prefab_attribute(proof, source.prefab(), PrefabAttribute::Name, Payload)?;
        }
        PrefabIssueKind::LibraryUnreadable { .. } => {
            if let Some(evidence) = source.link().and_then(PrefabLinkSource::unreadable) {
                sites.unreadable(proof, evidence)?;
            } else {
                proof.complete = false;
            }
        }
        PrefabIssueKind::Missing { .. }
        | PrefabIssueKind::Cycle { .. }
        | PrefabIssueKind::GuidMismatch { .. } => {
            let Some(object) = source.object() else {
                proof.complete = false;
                return Ok(());
            };
            let positioned = object.source();
            let node = positioned.map(|source| &source.transform.element);
            let reference = positioned.and_then(|source| source.reference.as_ref());
            sites.attribute(
                proof,
                object.prefab.file,
                node,
                reference.and_then(|reference| reference.name.as_ref()),
                "PrefabName",
                Payload,
            )?;
            if matches!(kind, PrefabIssueKind::GuidMismatch { .. }) {
                sites.attribute(
                    proof,
                    object.prefab.file,
                    node,
                    reference.and_then(|reference| reference.guid.as_ref()),
                    "PrefabGUID",
                    Payload,
                )?;
                if let Some(target) = source.link().and_then(PrefabLinkSource::target) {
                    sites.prefab_attribute(proof, target, PrefabAttribute::Id, Payload)?;
                } else {
                    proof.complete = false;
                }
            }
        }
        _ => {}
    }
    Ok(())
}
