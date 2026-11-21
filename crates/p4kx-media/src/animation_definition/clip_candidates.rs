// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::collections::HashSet;

use schemars::JsonSchema;
use serde::Serialize;

use super::definition_scope::allocation;
use crate::origin_contributions::OriginContributions;
use crate::{DirectoryContext, MediaError, MediaWriter};

#[path = "clip_candidate_join.rs"]
mod join;
#[path = "clip_candidate_payload.rs"]
mod payload;
#[path = "clip_candidate_routes.rs"]
mod routes;

pub(super) use payload::ClipContributor;

#[derive(Clone, Copy)]
pub(super) struct Site {
    pub(super) entry: usize,
    pub(super) indices: [u64; 2],
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum ArcKind {
    Database,
    Parameters,
    Space,
}

#[derive(Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "ClipContributorAliasKind")]
pub(super) enum AliasKind {
    NamedCaf,
    NamedDba,
    Wildcard,
}

#[derive(Clone, Copy)]
pub(super) enum Target {
    Caf { entry: usize },
    Dba { entry: usize, clip: u64 },
    BlendSpace { entry: usize },
    CombinedSpace { entry: usize },
}

impl Target {
    const fn space_entry(self) -> Option<usize> {
        match self {
            Self::BlendSpace { entry } | Self::CombinedSpace { entry } => Some(entry),
            Self::Caf { .. } | Self::Dba { .. } => None,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum Fact<'source> {
    Placement {
        ordinal: u64,
        parameters: usize,
        proof: &'source OriginContributions,
        ordinal_proof: &'source OriginContributions,
    },
    Binding {
        ordinal: u64,
        placement: u64,
        root: usize,
        proof: &'source OriginContributions,
    },
    Arc {
        from: usize,
        to: usize,
        kind: ArcKind,
        proof: &'source OriginContributions,
    },
    Readable {
        entry: usize,
    },
    Clip {
        site: Site,
        authored: &'source str,
        proof: &'source OriginContributions,
    },
    Alias {
        site: Site,
        ordinal: u64,
        kind: AliasKind,
        name: &'source str,
        target: Target,
        origins: AliasProofs<'source>,
    },
}

#[derive(Clone, Copy)]
pub(super) struct AliasProofs<'source> {
    pub(super) occurrence: &'source OriginContributions,
    pub(super) name: &'source OriginContributions,
    pub(super) site: &'source OriginContributions,
    pub(super) target: &'source OriginContributions,
    pub(super) target_clip: Option<&'source OriginContributions>,
}

struct Placement {
    ordinal: u64,
    parameters: usize,
    proof: OriginContributions,
    ordinal_proof: OriginContributions,
}

struct Binding {
    ordinal: u64,
    placement: u64,
    root: usize,
    proof: OriginContributions,
}

struct RouteArc {
    from: usize,
    to: usize,
    kind: ArcKind,
    proof: OriginContributions,
}

struct Clip {
    site: Site,
    authored: String,
    proof: OriginContributions,
}

struct Alias {
    site: Site,
    ordinal: u64,
    kind: AliasKind,
    name: String,
    target: Target,
    proof: OriginContributions,
    name_proof: OriginContributions,
    site_proof: OriginContributions,
    target_proof: OriginContributions,
    target_clip_proof: Option<OriginContributions>,
}

#[derive(Default)]
pub(crate) struct ClipCandidates(RefCell<Observed>);

#[derive(Default)]
struct Observed {
    placements: Vec<Placement>,
    bindings: Vec<Binding>,
    arcs: Vec<RouteArc>,
    readable: HashSet<usize>,
    clips: Vec<Clip>,
    aliases: Vec<Alias>,
}

impl ClipCandidates {
    pub(super) fn observe(
        &self,
        fact: Fact<'_>,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        if !writer.scoped_animation_definitions
            || (writer.animation_entity.is_none() && writer.animation_model.is_none())
        {
            return Ok(());
        }
        writer.checkpoint()?;
        let mut observed = self.0.try_borrow_mut().map_err(|_| borrowed())?;
        observed.record(fact, writer)
    }

    pub(super) fn write(
        &self,
        context: &DirectoryContext<'_, '_, '_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let observed = self.0.try_borrow().map_err(|_| borrowed())?;
        if observed.placements.is_empty()
            || observed.clips.is_empty()
            || observed.aliases.is_empty()
        {
            return Ok(());
        }
        join::write(&observed, context, writer)
    }
}

impl Observed {
    fn record(&mut self, fact: Fact<'_>, writer: &MediaWriter<'_>) -> Result<(), MediaError> {
        match fact {
            Fact::Placement {
                ordinal,
                parameters,
                proof,
                ordinal_proof,
            } => self.placement(ordinal, parameters, proof, ordinal_proof, writer),
            Fact::Binding {
                ordinal,
                placement,
                root,
                proof,
            } => self.binding(ordinal, placement, root, proof, writer),
            Fact::Arc {
                from,
                to,
                kind,
                proof,
            } => self.arc(from, to, kind, proof, writer),
            Fact::Readable { entry } => {
                self.readable
                    .try_reserve(1)
                    .map_err(|cause| allocation(writer, cause))?;
                self.readable.insert(entry);
                Ok(())
            }
            Fact::Clip {
                site,
                authored,
                proof,
            } => push(
                &mut self.clips,
                Clip {
                    site,
                    authored: copy_name(authored, writer)?,
                    proof: proof.clone(),
                },
                writer,
            ),
            Fact::Alias {
                site,
                ordinal,
                kind,
                name,
                target,
                origins,
            } => self.alias((site, ordinal, kind), name, target, origins, writer),
        }
    }

    fn placement(
        &mut self,
        ordinal: u64,
        parameters: usize,
        proof: &OriginContributions,
        ordinal_proof: &OriginContributions,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        push(
            &mut self.placements,
            Placement {
                ordinal,
                parameters,
                proof: proof.clone(),
                ordinal_proof: ordinal_proof.clone(),
            },
            writer,
        )
    }

    fn binding(
        &mut self,
        ordinal: u64,
        placement: u64,
        root: usize,
        proof: &OriginContributions,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        push(
            &mut self.bindings,
            Binding {
                ordinal,
                placement,
                root,
                proof: proof.clone(),
            },
            writer,
        )
    }

    fn arc(
        &mut self,
        from: usize,
        to: usize,
        kind: ArcKind,
        proof: &OriginContributions,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        push(
            &mut self.arcs,
            RouteArc {
                from,
                to,
                kind,
                proof: proof.clone(),
            },
            writer,
        )
    }

    fn alias(
        &mut self,
        (site, ordinal, kind): (Site, u64, AliasKind),
        name: &str,
        target: Target,
        origins: AliasProofs<'_>,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        push(
            &mut self.aliases,
            Alias {
                site,
                ordinal,
                kind,
                name: copy_name(name, writer)?,
                target,
                proof: origins.occurrence.clone(),
                name_proof: origins.name.clone(),
                site_proof: origins.site.clone(),
                target_proof: origins.target.clone(),
                target_clip_proof: origins.target_clip.cloned(),
            },
            writer,
        )
    }
}

fn copy_name(name: &str, writer: &MediaWriter<'_>) -> Result<String, MediaError> {
    let mut owned = String::new();
    owned
        .try_reserve_exact(name.len())
        .map_err(|cause| allocation(writer, cause))?;
    owned.push_str(name);
    Ok(owned)
}

fn push<T>(values: &mut Vec<T>, value: T, writer: &MediaWriter<'_>) -> Result<(), MediaError> {
    values
        .try_reserve(1)
        .map_err(|cause| allocation(writer, cause))?;
    values.push(value);
    Ok(())
}

fn borrowed() -> MediaError {
    MediaError::Contract("scoped animation clip candidates are already borrowed".into())
}
