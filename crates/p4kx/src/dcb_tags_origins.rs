// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use anyhow::Result;
use p4kx_extraction::{
    FieldOrigins, FieldPath, Guid, InputScope, SourceInventory, SourceUse, SourceUses, Tag, TagTree,
};

use super::{Listing, Selection};
use crate::forge_input::ForgeInput;

#[path = "dcb_tags_origin_capture.rs"]
mod capture;
#[path = "dcb_tags_origin_read.rs"]
mod read;

#[derive(Clone)]
struct Proof {
    uses: Vec<SourceUse>,
    complete: bool,
}

impl Default for Proof {
    fn default() -> Self {
        Self {
            uses: Vec::new(),
            complete: true,
        }
    }
}

impl Proof {
    fn one(used: SourceUse, complete: bool) -> Self {
        Self {
            uses: vec![used],
            complete,
        }
    }

    fn include(&mut self, other: &Self) {
        self.uses.extend(other.uses.iter().cloned());
        self.complete &= other.complete;
    }

    fn incomplete() -> Self {
        Self {
            complete: false,
            ..Self::default()
        }
    }
}

struct Node {
    index: u32,
    guid: Guid,
    parent: Option<usize>,
    listed: bool,
    qualification: Proof,
    incoming: Option<Proof>,
    guid_payload: SourceUse,
    name: Proof,
    path_name: Proof,
    legacy: Proof,
    children: Proof,
}

struct Counts {
    classification: Proof,
    readability: Proof,
    membership: Proof,
    topology: Proof,
    dangling: Proof,
    repeated: Proof,
}

/// Field origins from the actual observed read and selected Tag tree.
pub(crate) struct TagOrigins {
    tree: TagTree,
    uses: SourceUses,
    inventory: SourceInventory,
    nodes: Vec<Node>,
    counts: Counts,
}

impl TagOrigins {
    pub(crate) fn build(input: &ForgeInput) -> Result<Self> {
        let entry = input.entry();
        let forge = entry.open()?;
        let uses = SourceUses::new();
        let scope = uses.observe(input.source_input()?, InputScope::Payload)?;
        let mut capture = capture::Capture::default();
        let tree = TagTree::build_observed(&forge, &|| false, &mut |event| {
            capture.event(&scope, &forge, &event)
        })?
        .ok_or_else(|| anyhow::anyhow!("Tag tree was not completed"))?;
        let (nodes, counts) = capture.finish(&scope, forge.record_count(), &tree)?;
        let inventory = uses.freeze()?;
        Ok(Self {
            tree,
            uses,
            inventory,
            nodes,
            counts,
        })
    }

    pub(crate) const fn tree(&self) -> &TagTree {
        &self.tree
    }

    pub(crate) const fn inventory(&self) -> &SourceInventory {
        &self.inventory
    }

    pub(crate) fn summary(&self, listing: &Listing<'_>) -> Result<FieldOrigins> {
        let mut fields = FieldOrigins::new();
        self.insert(&mut fields, "/tag_count", &self.counts.membership)?;
        let shown = self.shown_count(listing)?;
        self.insert(&mut fields, "/shown_count", &shown)?;
        self.insert(
            &mut fields,
            "/issues/unclassified",
            &self.counts.classification,
        )?;
        self.insert(&mut fields, "/issues/unreadable", &self.counts.readability)?;
        self.insert(&mut fields, "/issues/duplicate", &self.counts.membership)?;
        self.insert(&mut fields, "/issues/too_deep", &Proof::default())?;
        self.insert(&mut fields, "/issues/too_long", &Proof::default())?;
        let mut dangling = self.counts.topology.clone();
        dangling.include(&self.counts.dangling);
        self.insert(&mut fields, "/issues/dangling", &dangling)?;
        let mut repeated = self.counts.topology.clone();
        repeated.include(&self.counts.repeated);
        self.insert(&mut fields, "/issues/repeated", &repeated)?;
        dangling.include(&self.counts.repeated);
        self.insert(&mut fields, "/issues/unlisted", &dangling)?;
        Ok(fields)
    }

    fn shown_count(&self, listing: &Listing<'_>) -> Result<Proof> {
        match listing.selection {
            Selection::All => Ok(self.counts.membership.clone()),
            Selection::Guid => {
                let mut shown = Proof::default();
                for row in &listing.rows {
                    let node = self.node(row.tag)?;
                    shown.include(&node.qualification);
                    shown.include(&node.children);
                    if row.indent > 0 {
                        shown.include(&Self::incoming(node));
                    }
                }
                Ok(shown)
            }
            Selection::Path => {
                let mut shown = self.counts.topology.clone();
                for node in &self.nodes {
                    shown.include(&node.path_name);
                }
                Ok(shown)
            }
        }
    }

    pub(crate) fn row(&self, position: usize, flat: bool) -> Result<FieldOrigins> {
        let tag = self
            .tree
            .at(position)
            .ok_or_else(|| anyhow::anyhow!("missing Tag in the observed tree"))?;
        let node = self.node(tag)?;
        let mut fields = FieldOrigins::new();
        fields.insert(
            FieldPath::parse("/guid")?,
            self.uses
                .origin(std::slice::from_ref(&node.guid_payload), true),
        )?;
        self.insert(&mut fields, "/name", &node.name)?;
        self.insert(&mut fields, "/legacy_id", &node.legacy)?;

        let mut topology = Proof::default();
        let mut names = Proof::default();
        for ancestor in tag.ancestors_and_self() {
            let ancestor = self.node(ancestor)?;
            topology.include(&ancestor.qualification);
            topology.include(&Self::incoming(ancestor));
            names.include(&ancestor.path_name);
        }
        if flat {
            self.insert(&mut fields, "/depth", &topology)?;
            let mut parent = Self::incoming(node);
            if let Some(selected) = tag.parent() {
                parent.uses.push(self.node(selected)?.guid_payload.clone());
            }
            self.insert(&mut fields, "/parent", &parent)?;
        }
        topology.include(&names);
        self.insert(&mut fields, "/path", &topology)?;

        let mut children = node.children.clone();
        for (index, child) in tag.children().enumerate() {
            let child = self.node(child)?;
            let mut relation = Self::incoming(child);
            if flat {
                relation.uses.push(child.guid_payload.clone());
            } else {
                relation.include(&child.qualification);
            }
            self.insert(&mut fields, &format!("/children/{index}"), &relation)?;
            children.include(&relation);
        }
        self.insert(&mut fields, "/children", &children)?;
        Ok(fields)
    }

    fn node(&self, tag: Tag<'_>) -> Result<&Node> {
        self.nodes
            .get(tag.position())
            .ok_or_else(|| anyhow::anyhow!("missing selected Tag origin"))
    }

    fn incoming(node: &Node) -> Proof {
        node.incoming.clone().unwrap_or_else(Proof::incomplete)
    }

    fn insert(&self, fields: &mut FieldOrigins, path: &str, proof: &Proof) -> Result<()> {
        fields.insert(
            FieldPath::parse(path)?,
            self.uses.origin(&proof.uses, proof.complete),
        )?;
        Ok(())
    }
}
