// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_assembly::{Assembly, EntityGlow, EntityGlowLink, GlowEffectKind, GlowSelection, Part};
use p4kx_extraction::{
    DataForge, DataForgeEntry, EntityMaterialEffects, FieldOrigins, SourceInventory,
};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::Value;

use crate::{Manifest, PackageError, PackageResult, PackageWriteContext};

#[path = "package_glow_bindings.rs"]
mod bindings;

use bindings::DrawBinding;

#[derive(Clone, Copy)]
pub(crate) struct Source<'a> {
    pub(crate) entry: &'a DataForgeEntry,
    pub(crate) forge: &'a DataForge<'a>,
}

#[derive(Serialize, JsonSchema)]
pub(crate) struct PackageGlow {
    declarations: Vec<Declaration>,
    owners: Vec<Owner>,
}

#[derive(Serialize, JsonSchema)]
struct Declaration {
    record_index: u32,
    guid: String,
    components: Vec<Component>,
    provenance: SourceInventory,
}

#[derive(Serialize, JsonSchema)]
struct Component {
    ordinal: usize,
    source_type: String,
    authored: Value,
    field_origins: FieldOrigins,
}

#[derive(Serialize, JsonSchema)]
struct Owner {
    node: usize,
    declaration: usize,
    links: Vec<Link>,
    provenance: SourceInventory,
}

#[derive(Serialize, JsonSchema)]
struct Link {
    component: usize,
    kind: Kind,
    effect: usize,
    ordinal: usize,
    selection: Selection,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum Kind {
    Tag,
    Trigger,
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
enum Selection {
    Absent,
    InvalidSelector,
    MissingModel,
    UnobservedNodes,
    Unmatched,
    Ambiguous { nodes: Vec<bindings::CandidateNode> },
    NoDrawableSubsets,
    Bound { subsets: Vec<DrawBinding> },
    Unsupported,
}

pub(crate) fn attach(
    manifest: &mut Manifest,
    assembly: &Assembly,
    context: &PackageWriteContext<'_>,
) -> PackageResult<()> {
    if let Some(source) = context.glow_source {
        manifest.entity_glow = capture(manifest, assembly, context, source)?;
    }
    Ok(())
}

fn capture(
    manifest: &Manifest,
    assembly: &Assembly,
    context: &PackageWriteContext<'_>,
    source: Source<'_>,
) -> PackageResult<Option<Value>> {
    let mut declarations = Vec::new();
    let mut owners = Vec::new();
    let mut declared = BTreeMap::new();
    for (node, part) in assembly.parts().iter().enumerate() {
        context.check()?;
        let Some(index) = part.entity_record_index() else {
            continue;
        };
        let candidate = assembly
            .entity_glow(node, source.entry, source.forge)
            .map_err(error)?;
        if candidate.links.is_empty() {
            continue;
        }
        let EntityGlow {
            authored,
            links,
            model_sources,
            ..
        } = candidate;
        let declaration = *declared.entry(index).or_insert_with(|| {
            declarations.push(Declaration::from(authored));
            declarations.len() - 1
        });
        let owner = Owner::read(node, declaration, part, model_sources, links, manifest)?;
        owners.push(owner);
    }
    if owners.is_empty() {
        return Ok(None);
    }
    serde_json::to_value(PackageGlow {
        declarations,
        owners,
    })
    .map(Some)
    .map_err(error)
}

impl From<EntityMaterialEffects> for Declaration {
    fn from(source: EntityMaterialEffects) -> Self {
        Self {
            record_index: source.record_index,
            guid: source.guid.to_string(),
            components: source
                .components
                .into_iter()
                .map(|component| Component {
                    ordinal: component.component,
                    source_type: component.source_type,
                    authored: component.declaration,
                    field_origins: component.field_origins,
                })
                .collect(),
            provenance: source.provenance,
        }
    }
}

impl Owner {
    fn read(
        node: usize,
        declaration: usize,
        part: &Part,
        model_sources: &[p4kx_extraction::ModelArchiveSource],
        links: Vec<EntityGlowLink>,
        manifest: &Manifest,
    ) -> PackageResult<Self> {
        let evidence = bindings::Evidence::of(part, model_sources, manifest, node)?;
        let links = links
            .into_iter()
            .map(|link| Link::of(link, &evidence))
            .collect::<PackageResult<_>>()?;
        Ok(Self {
            node,
            declaration,
            links,
            provenance: evidence.finish()?,
        })
    }
}

impl Link {
    fn of(link: EntityGlowLink, evidence: &bindings::Evidence<'_>) -> PackageResult<Self> {
        let kind = match link.kind {
            GlowEffectKind::Tag => Kind::Tag,
            GlowEffectKind::Trigger => Kind::Trigger,
            _ => {
                return Err(error(std::io::Error::other(
                    "unknown authored Glow link class",
                )));
            }
        };
        let selection = match link.selection {
            GlowSelection::Absent => Selection::Absent,
            GlowSelection::InvalidSelector => Selection::InvalidSelector,
            GlowSelection::MissingModel => Selection::MissingModel,
            GlowSelection::UnobservedNodes => Selection::UnobservedNodes,
            GlowSelection::Unmatched => Selection::Unmatched,
            GlowSelection::Ambiguous { nodes } => Selection::Ambiguous {
                nodes: nodes
                    .into_iter()
                    .map(|node| evidence.candidate(node))
                    .collect::<PackageResult<_>>()?,
            },
            GlowSelection::NoDrawableSubsets => Selection::NoDrawableSubsets,
            GlowSelection::Bound(subsets) => Selection::Bound {
                subsets: subsets
                    .into_iter()
                    .map(|subset| evidence.bind(subset))
                    .collect::<PackageResult<_>>()?,
            },
            _ => Selection::Unsupported,
        };
        Ok(Self {
            component: link.component,
            kind,
            effect: link.effect,
            ordinal: link.link,
            selection,
        })
    }
}

pub(crate) fn error(error: impl std::error::Error + Send + Sync + 'static) -> PackageError {
    PackageError::GlowEvidence(Box::new(error))
}
