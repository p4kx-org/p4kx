// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::OnceCell;

use p4kx_extraction::cryxml::{SourceAttributePosition, SourceDeclaration, SourceElement};
use p4kx_extraction::{DecimalU64, SourceUseRole};

use super::graph_source::GraphRole;
use super::origins;
use super::payload::{self as wire, DefinitionPayload};
use super::projection::{DefinitionProjection, Failure};
use super::sub_database_scope::{self, Scope};
use super::sub_database_selectors_payload::{
    self as payload, Resolution, ScopeState, SelectorKind, SelectorState,
};
use crate::audio_control_payload::AtlPosition;
use crate::origin_contributions::OriginContributions;
use crate::xml_source_projection::element_position;
use crate::{MediaError, MediaWriter, Status, Subresource};

pub(super) struct Scopes<'document> {
    root: SourceElement<'document>,
    tags: OnceCell<Scope>,
    fragments: OnceCell<Scope>,
}

#[derive(Clone, Copy)]
pub(super) struct Site {
    pub(super) branch: u64,
    pub(super) position: AtlPosition,
    pub(super) selector: u64,
    pub(super) declaration: AtlPosition,
}

pub(super) struct BranchSelector {
    kind: SelectorKind,
    positions: AttributePositions,
    evidence: OriginContributions,
}

enum AttributePositions {
    Missing,
    Single {
        position: SourceAttributePosition,
        empty: bool,
    },
    Repeated,
}

impl<'document> Scopes<'document> {
    pub(super) const fn new(root: SourceElement<'document>) -> Self {
        Self {
            root,
            tags: OnceCell::new(),
            fragments: OnceCell::new(),
        }
    }

    fn scope(
        &self,
        kind: SelectorKind,
        projection: &DefinitionProjection<'_, '_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<&Scope, Failure> {
        let slot = match kind {
            SelectorKind::Tags => &self.tags,
            SelectorKind::FragmentName => &self.fragments,
        };
        if let Some(scope) = slot.get() {
            return Ok(scope);
        }
        let scope = Scope::read(projection, self.root, kind, writer)?;
        Ok(slot.get_or_init(|| scope))
    }

    pub(super) fn tokens(
        &self,
        position: Site,
        selector: &BranchSelector,
        declaration: &SourceDeclaration<'_>,
        declaration_ordinal: u64,
        projection: &DefinitionProjection<'_, '_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), Failure> {
        let AttributePositions::Single {
            position: expected,
            empty: false,
        } = &selector.positions
        else {
            return Ok(());
        };
        if declaration.position() != expected {
            return Ok(());
        }
        let scope = self.scope(selector.kind, projection, writer)?;
        let original = origins::position(
            projection.input,
            position.declaration,
            SourceUseRole::Selector,
            writer,
        )?;
        let branch = branch_evidence(projection, position.position, &original, writer)?;
        let spelling = declaration.value();
        let query = Query {
            projection,
            site: position,
            branch,
            declaration: declaration_ordinal,
            original,
        };
        match selector.kind {
            SelectorKind::Tags => {
                for (ordinal, token) in spelling.split('+').enumerate() {
                    query.write(token, ordinal as u64, scope, writer)?;
                }
            }
            SelectorKind::FragmentName => query.write(spelling, 0, scope, writer)?,
        }
        Ok(())
    }
}

impl BranchSelector {
    pub(super) fn of(
        role: Option<GraphRole>,
        node: SourceElement<'_>,
        projection: &DefinitionProjection<'_, '_>,
        writer: &MediaWriter<'_>,
    ) -> Result<Option<Self>, Failure> {
        let (kind, name) = match role {
            Some(GraphRole::SubDatabase) => (SelectorKind::Tags, "Tags"),
            Some(GraphRole::SubDatabaseFragment) => (SelectorKind::FragmentName, "Name"),
            _ => return Ok(None),
        };
        let mut capture = writer.capture_origins();
        let mut positions = AttributePositions::Missing;
        for attr in node
            .attributes()
            .filter(|attr| attr.namespace().is_none() && attr.name() == name)
        {
            sub_database_scope::metadata(projection, writer)?;
            let position = crate::xml_source_projection::declaration_position(attr.position())?;
            capture.include(
                &origins::position(projection.input, position, SourceUseRole::Selector, writer)?,
                writer,
            )?;
            positions = match positions {
                AttributePositions::Missing => AttributePositions::Single {
                    position: attr.position().clone(),
                    empty: attr.value().is_empty(),
                },
                _ => AttributePositions::Repeated,
            };
        }
        Ok(Some(Self {
            kind,
            positions,
            evidence: capture.finish(),
        }))
    }

    pub(super) fn write(
        &self,
        scopes: &Scopes<'_>,
        projection: &DefinitionProjection<'_, '_>,
        node: SourceElement<'_>,
        site: Site,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), Failure> {
        let state = match self.positions {
            AttributePositions::Missing => SelectorState::Missing,
            AttributePositions::Repeated => SelectorState::Repeated,
            AttributePositions::Single { empty: true, .. } => SelectorState::Empty,
            AttributePositions::Single { empty: false, .. } => SelectorState::Present,
        };
        let proof = origins::position(
            projection.input,
            element_position(&node.position())?,
            SourceUseRole::Selector,
            writer,
        )?;
        let branch = branch_evidence(projection, site.position, &proof, writer)?;
        let mut prepared = origins::record(projection.input, branch.clone(), writer)?;
        let mut selection = writer.capture_origins();
        selection.include(&branch, writer)?;
        selection.include(&self.evidence, writer)?;
        let selection = selection.finish();
        crate::audio_record_origins::field(
            &mut prepared,
            "/data/payload/state",
            selection.clone(),
            writer,
        )?;
        let scope = if matches!(state, SelectorState::Present) {
            let scope = scopes.scope(self.kind, projection, writer)?;
            let mut result = writer.capture_origins();
            result.include(&selection, writer)?;
            result.include(&scope.evidence, writer)?;
            crate::audio_record_origins::field(
                &mut prepared,
                "/data/payload/scope",
                result.finish(),
                writer,
            )?;
            Some(scope.state)
        } else {
            None
        };
        prepared.write(
            writer,
            &Subresource::new(wire::SUB_DATABASE_SELECTOR, vec![site.selector]),
            Status::Complete,
            &DefinitionPayload::SubDatabaseSelector(payload::Selector {
                branch: DecimalU64::new(site.branch),
                kind: self.kind,
                position: element_position(&node.position())?,
                state,
                scope,
            }),
            &[],
        )?;
        Ok(())
    }
}

struct Query<'a, 'context, 'source> {
    projection: &'a DefinitionProjection<'context, 'source>,
    site: Site,
    branch: OriginContributions,
    declaration: u64,
    original: OriginContributions,
}

impl Query<'_, '_, '_> {
    fn write(
        &self,
        spelling: &str,
        ordinal: u64,
        scope: &Scope,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        sub_database_scope::metadata(self.projection, writer)?;
        let mut proof = writer.capture_origins();
        proof.include(&self.branch, writer)?;
        let mut candidates = &[][..];
        let resolution = if spelling.is_empty() {
            Resolution::InvalidToken
        } else if !spelling.is_ascii() {
            Resolution::UnsupportedSpelling
        } else {
            proof.include(&scope.evidence, writer)?;
            candidates = scope.candidates(spelling);
            match (candidates, scope.state) {
                ([_, _, ..], _) => Resolution::Ambiguous,
                (_, state) if !matches!(state, ScopeState::Complete) => Resolution::Incomplete,
                ([_], _) => Resolution::Resolved,
                _ => Resolution::Missing,
            }
        };
        let proof = proof.finish();
        let mut prepared = origins::record(self.projection.input, self.original.clone(), writer)?;
        for field in ["/data/payload/resolution", "/data/payload/candidate_count"] {
            crate::audio_record_origins::field(&mut prepared, field, proof.clone(), writer)?;
        }
        let key = [self.site.selector, self.declaration, ordinal];
        prepared.write(
            writer,
            &Subresource::new(wire::SUB_DATABASE_TOKEN, key.to_vec()),
            Status::Complete,
            &DefinitionPayload::SubDatabaseToken(payload::Token {
                spelling,
                position: self.site.declaration,
                resolution,
                candidate_count: DecimalU64::new(candidates.len() as u64),
            }),
            &[],
        )?;
        for (index, definition) in candidates.iter().enumerate() {
            self.candidate(key, index as u64, definition, scope, writer)?;
        }
        Ok(())
    }

    fn candidate(
        &self,
        [element, declaration, token]: [u64; 3],
        index: u64,
        definition: &sub_database_scope::Candidate,
        scope: &Scope,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        sub_database_scope::metadata(self.projection, writer)?;
        let mut proof = writer.capture_origins();
        proof.include(&self.branch, writer)?;
        proof.include(&scope.evidence, writer)?;
        let element_proof = origins::position(
            &definition.source.input,
            definition.position,
            SourceUseRole::Payload,
            writer,
        )?;
        let name_proof = origins::position(
            &definition.source.input,
            definition.name_position,
            SourceUseRole::Payload,
            writer,
        )?;
        proof.include(&element_proof, writer)?;
        proof.include(&name_proof, writer)?;
        let mut prepared = origins::record(self.projection.input, proof.finish(), writer)?;
        for (field, origin) in [
            ("/data/payload/position", element_proof),
            ("/data/payload/name", name_proof.clone()),
            ("/data/payload/name_position", name_proof),
        ] {
            crate::audio_record_origins::field(&mut prepared, field, origin, writer)?;
        }
        prepared.write(
            writer,
            &Subresource::new(
                wire::SUB_DATABASE_CANDIDATE,
                vec![element, declaration, token, index],
            ),
            Status::Complete,
            &DefinitionPayload::SubDatabaseCandidate(payload::Candidate {
                target: &definition.source.target,
                name: &definition.name,
                position: definition.position,
                name_position: definition.name_position,
            }),
            &[],
        )
    }
}

fn branch_evidence(
    projection: &DefinitionProjection<'_, '_>,
    branch: AtlPosition,
    own: &OriginContributions,
    writer: &MediaWriter<'_>,
) -> Result<OriginContributions, MediaError> {
    let mut proof = writer.capture_origins();
    proof.include(own, writer)?;
    proof.include(
        &origins::position(projection.input, branch, SourceUseRole::Selector, writer)?,
        writer,
    )?;
    Ok(proof.finish())
}
