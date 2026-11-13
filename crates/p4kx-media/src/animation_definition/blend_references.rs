// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DecimalU64, SourceUseRole};

use super::blend_reference_index::{self, Alias, BlendIndex, Example, Query};
use super::blend_reference_payload::{
    AliasDisposition, BlendCandidate, BlendReference, CandidateBasis, Interpretation,
    OrdinalDisposition,
};
use super::origins;
use super::payload::{self as wire, DefinitionPayload};
use super::projection::DefinitionProjection;
use crate::audio_record_origins as records;
use crate::origin_contributions::OriginContributions;
use crate::{MediaError, MediaWriter, Status, Subresource};

struct Selection<'a, 'source> {
    ordinal: OrdinalDisposition,
    ordinal_target: Option<Example>,
    aliases: &'a [Alias<'source>],
    exact: u64,
    variants: u64,
    exact_matches_ordinal: bool,
}

impl<'a, 'source> Selection<'a, 'source> {
    fn resolve(
        index: &'a BlendIndex<'source>,
        query: &Query<'_>,
        projection: &DefinitionProjection<'_, '_>,
        writer: &MediaWriter<'_>,
    ) -> Result<Self, MediaError> {
        let (ordinal, ordinal_target) = numeric(index, query);
        let aliases = index.aliases(query.value);
        let mut selection = Self {
            ordinal,
            ordinal_target,
            aliases,
            exact: 0,
            variants: 0,
            exact_matches_ordinal: false,
        };
        let budget = projection.context.directory.input_state()?.budget;
        for alias in aliases {
            budget.metadata(0, 1, writer.limits(), writer.checkpoint)?;
            if !blend_reference_index::accepts_alias(query.kind, alias.target.kind) {
                continue;
            }
            if alias.spelling == query.value {
                selection.exact += 1;
                selection.exact_matches_ordinal |=
                    ordinal_target.is_some_and(|target| target.index == alias.target.index);
            } else {
                selection.variants += 1;
            }
        }
        Ok(selection)
    }

    const fn alias(&self) -> AliasDisposition {
        match (self.exact, self.variants) {
            (1, _) => AliasDisposition::Exact,
            (2.., _) => AliasDisposition::RepeatedExact,
            (0, 1) => AliasDisposition::CaseVariant,
            (0, 2..) => AliasDisposition::RepeatedCaseVariant,
            (0, 0) => AliasDisposition::Missing,
        }
    }

    const fn interpretation(&self) -> Interpretation {
        match (self.exact, self.variants, self.ordinal_target.is_some()) {
            (1, _, true) if self.exact_matches_ordinal => Interpretation::Agreement,
            (1, _, true) | (2.., _, _) => Interpretation::Conflict,
            (1, _, false) => Interpretation::ExactAliasOnly,
            (0, 0, true) => Interpretation::OrdinalOnly,
            (0, 1.., true) => Interpretation::OrdinalWithCaseVariant,
            (0, 1.., false) => Interpretation::TentativeCaseVariant,
            (0, 0, false) => Interpretation::Unresolved,
        }
    }

    fn candidates(&self, query: &Query<'_>) -> impl Iterator<Item = Candidate<'source>> {
        let ordinal = self.ordinal_target.into_iter().map(Candidate::Ordinal);
        let aliases = self.aliases.iter().copied().filter_map(|alias| {
            if !blend_reference_index::accepts_alias(query.kind, alias.target.kind) {
                return None;
            }
            match (alias.spelling == query.value, self.exact) {
                (true, _) => Some(Candidate::Alias {
                    alias,
                    basis: CandidateBasis::ExactAlias,
                }),
                (false, 0) => Some(Candidate::Alias {
                    alias,
                    basis: CandidateBasis::CaseVariantAlias,
                }),
                (false, _) => None,
            }
        });
        ordinal.chain(aliases)
    }
}

enum Candidate<'source> {
    Ordinal(Example),
    Alias {
        alias: Alias<'source>,
        basis: CandidateBasis,
    },
}

impl Candidate<'_> {
    fn payload(&self) -> BlendCandidate {
        let (basis, target, alias) = match self {
            Self::Ordinal(target) => (CandidateBasis::LocalOrdinal, *target, None),
            Self::Alias { alias, basis } => (*basis, alias.target, Some(*alias)),
        };
        BlendCandidate {
            basis,
            target_element_index: DecimalU64::new(target.index),
            target_kind: target.kind,
            target_position: target.position,
            alias_declaration_ordinal: alias.map(|alias| DecimalU64::new(alias.ordinal)),
            alias_position: alias.map(|alias| alias.position),
        }
    }
}

pub(super) fn write(
    projection: &DefinitionProjection<'_, '_>,
    index: &BlendIndex<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    if index.queries.is_empty() {
        return Ok(());
    }
    let inventory = origins::file(projection.input, projection.context.source.size, writer)?;
    let budget = projection.context.directory.input_state()?.budget;
    for query in &index.queries {
        budget.metadata(0, 1, writer.limits(), writer.checkpoint)?;
        let selection = Selection::resolve(index, query, projection, writer)?;
        let selector = originates(projection, query.position, writer)?;
        let ordinal = ordinal_proof(
            projection,
            index,
            query,
            selection.ordinal,
            &selector,
            writer,
        )?;
        write_reference(
            projection, query, &selection, &inventory, &ordinal, &selector, writer,
        )?;
        for (position, candidate) in selection.candidates(query).enumerate() {
            budget.metadata(0, 1, writer.limits(), writer.checkpoint)?;
            let context = match &candidate {
                Candidate::Ordinal(_) => &ordinal,
                Candidate::Alias { .. } => &selector,
            };
            write_candidate(
                projection,
                query,
                position as u64,
                candidate.payload(),
                context,
                writer,
            )?;
        }
    }
    Ok(())
}

fn numeric(index: &BlendIndex<'_>, query: &Query<'_>) -> (OrdinalDisposition, Option<Example>) {
    let value = query.value.trim_ascii();
    let digits = value.strip_prefix(['+', '-']).unwrap_or(value);
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return (OrdinalDisposition::NonInteger, None);
    }
    if !index.ordinal_layout_supported() {
        return (OrdinalDisposition::UnavailableLayout, None);
    }
    let target = value
        .parse::<usize>()
        .ok()
        .and_then(|ordinal| index.ordinal_target(query.kind, ordinal))
        .copied();
    if target.is_some() {
        (OrdinalDisposition::Candidate, target)
    } else {
        (OrdinalDisposition::OutOfRange, None)
    }
}

fn write_reference(
    projection: &DefinitionProjection<'_, '_>,
    query: &Query<'_>,
    selection: &Selection<'_, '_>,
    inventory: &OriginContributions,
    ordinal: &OriginContributions,
    selector: &OriginContributions,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut alias = writer.capture_origins();
    alias.include(selector, writer)?;
    alias.include(inventory, writer)?;
    let alias = alias.finish();
    let mut interpretation = writer.capture_origins();
    interpretation.include(ordinal, writer)?;
    interpretation.include(&alias, writer)?;
    let mut prepared = origins::record(projection.input, selector.clone(), writer)?;
    for (address, proof) in [
        ("/data/payload/ordinal", ordinal.clone()),
        ("/data/payload/alias", alias),
        ("/data/payload/interpretations", interpretation.finish()),
    ] {
        records::field(&mut prepared, address, proof, writer)?;
    }
    prepared.write(
        writer,
        &Subresource::new(wire::BLEND_REFERENCE, query.indices.to_vec()),
        Status::Complete,
        &DefinitionPayload::BlendSpaceReference(BlendReference {
            kind: query.kind,
            attribute: query.attribute,
            value: query.value,
            lexeme: query.lexeme,
            position: query.position,
            ordinal: selection.ordinal,
            alias: selection.alias(),
            interpretations: selection.interpretation(),
        }),
        &[],
    )
}

fn write_candidate(
    projection: &DefinitionProjection<'_, '_>,
    query: &Query<'_>,
    ordinal: u64,
    payload: BlendCandidate,
    selector: &OriginContributions,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut proof = writer.capture_origins();
    proof.include(selector, writer)?;
    proof.include(
        &originates(projection, payload.target_position, writer)?,
        writer,
    )?;
    if let Some(alias) = payload.alias_position {
        proof.include(&originates(projection, alias, writer)?, writer)?;
    }
    let [element, declaration] = query.indices;
    origins::record(projection.input, proof.finish(), writer)?.write(
        writer,
        &Subresource::new(wire::BLEND_CANDIDATE, vec![element, declaration, ordinal]),
        Status::Complete,
        &DefinitionPayload::BlendSpaceCandidate(payload),
        &[],
    )
}

fn ordinal_proof(
    projection: &DefinitionProjection<'_, '_>,
    index: &BlendIndex<'_>,
    query: &Query<'_>,
    disposition: OrdinalDisposition,
    selector: &OriginContributions,
    writer: &MediaWriter<'_>,
) -> Result<OriginContributions, MediaError> {
    if matches!(disposition, OrdinalDisposition::NonInteger) {
        return Ok(selector.clone());
    }
    let mut proof = writer.capture_origins();
    proof.include(selector, writer)?;
    if !index.ordinal_layout_supported() {
        proof.include(
            &origins::file(projection.input, projection.context.source.size, writer)?,
            writer,
        )?;
    }
    for position in index.ordinal_sites(query.kind) {
        proof.include(&originates(projection, position, writer)?, writer)?;
    }
    Ok(proof.finish())
}

fn originates(
    projection: &DefinitionProjection<'_, '_>,
    position: crate::audio_control_payload::AtlPosition,
    writer: &MediaWriter<'_>,
) -> Result<OriginContributions, MediaError> {
    origins::position(projection.input, position, SourceUseRole::Selector, writer)
}
