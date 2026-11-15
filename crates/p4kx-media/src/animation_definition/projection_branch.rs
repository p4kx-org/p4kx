// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::cryxml::{SourceDeclaration, SourceElement};

use super::super::graph_source::GraphRole;
use super::super::sub_database_selectors::{BranchSelector, Scopes, Site as SelectorSite};
use super::super::timeline::TimelineNode;
use super::{DefinitionProjection, ElementSite, Failure};
use crate::MediaWriter;
use crate::audio_control_payload::{AtlElement, AtlLocation, AtlPosition};

#[derive(Clone, Copy)]
pub(super) struct BranchSite {
    index: u64,
    position: AtlPosition,
}

impl BranchSite {
    const fn at(
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

pub(super) struct AttachedSelector<'a, 'document> {
    selector: BranchSelector,
    scopes: &'a Scopes<'document>,
    site: SelectorSite,
}

impl AttachedSelector<'_, '_> {
    pub(super) fn observe(
        &self,
        projection: &DefinitionProjection<'_, '_>,
        declaration: &SourceDeclaration<'_>,
        (ordinal, position): (u64, AtlPosition),
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), Failure> {
        let site = SelectorSite {
            declaration: position,
            ..self.site
        };
        self.scopes.tokens(
            site,
            &self.selector,
            declaration,
            ordinal,
            projection,
            writer,
        )
    }
}

pub(super) fn select<'a, 'document>(
    projection: &DefinitionProjection<'_, '_>,
    scopes: &'a Scopes<'document>,
    element: SourceElement<'_>,
    (graph, parent): (Option<GraphRole>, Option<BranchSite>),
    ElementSite { index, position }: ElementSite,
    writer: &mut MediaWriter<'_>,
) -> Result<(Option<BranchSite>, Option<AttachedSelector<'a, 'document>>), Failure> {
    let branch = BranchSite::at(graph, index, position, parent);
    let selector = BranchSelector::of(graph, element, projection, writer)?;
    let (Some(selector), Some(owner)) = (selector, branch) else {
        return Ok((branch, None));
    };
    let site = SelectorSite {
        branch: owner.index,
        position: owner.position,
        selector: index,
        declaration: position,
    };
    selector.write(scopes, projection, element, site, writer)?;
    Ok((
        branch,
        Some(AttachedSelector {
            selector,
            scopes,
            site,
        }),
    ))
}

pub(super) fn positioned_element(
    projection: &DefinitionProjection<'_, '_>,
    element: SourceElement<'_>,
    qualified_name: &str,
    location: AtlLocation,
    site: ElementSite,
    roles: (TimelineNode, Option<GraphRole>),
    writer: &mut MediaWriter<'_>,
) -> Result<(), Failure> {
    let ElementSite { index, position } = site;
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
