// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::cryxml::SourceElement;

use super::super::graph_source::GraphRole;
use super::super::sub_database_selectors::{BranchSelector, Scopes, Site as SelectorSite};
use super::super::timeline::{TimelineIdentity, TimelineNode};
use super::{DefinitionProjection, Failure};
use crate::MediaWriter;
use crate::audio_control_payload::{AtlElement, AtlLocation, AtlPosition};

#[derive(Clone, Copy)]
pub(super) struct BranchSite {
    index: u64,
    position: AtlPosition,
}

impl BranchSite {
    pub(super) const fn for_element(
        graph: Option<GraphRole>,
        index: u64,
        position: AtlPosition,
        parent: Option<Self>,
    ) -> Option<Self> {
        if matches!(graph, Some(GraphRole::SubDatabase)) {
            Some(Self { index, position })
        } else {
            parent
        }
    }
}

pub(super) type AttachedSelector<'a, 'document> =
    (&'a BranchSelector, &'a Scopes<'document>, SelectorSite);

pub(super) fn attach_selector<'a, 'document>(
    projection: &DefinitionProjection<'_, '_>,
    scopes: &'a Scopes<'document>,
    element: SourceElement<'_>,
    selector: Option<&'a BranchSelector>,
    branch: Option<BranchSite>,
    (index, position): (u64, AtlPosition),
    writer: &mut MediaWriter<'_>,
) -> Result<Option<AttachedSelector<'a, 'document>>, Failure> {
    let (Some(selector), Some(branch)) = (selector, branch) else {
        return Ok(None);
    };
    let site = SelectorSite {
        branch: branch.index,
        position: branch.position,
        selector: index,
        declaration: position,
    };
    selector.write(scopes, projection, element, site, writer)?;
    Ok(Some((selector, scopes, site)))
}

pub(super) fn positioned_element(
    projection: &DefinitionProjection<'_, '_>,
    element: SourceElement<'_>,
    qualified_name: &str,
    location: AtlLocation,
    site: TimelineIdentity,
    roles: (TimelineNode, Option<GraphRole>),
    writer: &mut MediaWriter<'_>,
) -> Result<(), Failure> {
    let TimelineIdentity { index, position } = site;
    let (timeline, graph) = roles;
    projection.write_element(
        AtlElement {
            qualified_name,
            namespace: element.namespace(),
            location: location.clone(),
            position,
        },
        index,
        writer,
    )?;
    timeline.write(projection, element, index, position, writer)?;
    if let Some(graph) = graph {
        graph.write(
            projection,
            qualified_name,
            index,
            location,
            position,
            writer,
        )?;
    }
    Ok(())
}
