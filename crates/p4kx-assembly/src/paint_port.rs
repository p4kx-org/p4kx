// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_dataforge::{Tag, TagTree};

/// The offered and required tags of one typed paint port.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct PaintPort {
    port_tags: Vec<String>,
    required_tags: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default)]
struct PaintOfferSources {
    port: bool,
    vehicle: bool,
}

/// The actual sources selected by a successful paint compatibility check.
#[derive(Debug)]
pub struct PaintPortMatch<'tree> {
    sources: PaintOfferSources,
    ancestries: Vec<PaintTagAncestry<'tree>>,
}

impl<'tree> PaintPortMatch<'tree> {
    /// Borrows the actually used ancestry segments in requirement order.
    /// Equal tag names require no ancestry.
    #[must_use]
    pub fn ancestries(&self) -> &[PaintTagAncestry<'tree>] {
        &self.ancestries
    }

    /// Reports whether a paint requirement used the optional port tags.
    #[must_use]
    pub const fn uses_port_tags(&self) -> bool {
        self.sources.port
    }

    /// Reports whether a paint requirement used the vehicle tags.
    #[must_use]
    pub const fn uses_vehicle_tags(&self) -> bool {
        self.sources.vehicle
    }
}

/// The selected original Tag records behind one descendant match.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaintTagAncestry<'tree> {
    offered: Tag<'tree>,
    required: Tag<'tree>,
}

impl<'tree> PaintTagAncestry<'tree> {
    /// Returns the physical Tag selected for the offered name.
    #[must_use]
    pub const fn offered(&self) -> Tag<'tree> {
        self.offered
    }

    /// Returns the physical Tag selected for the required name.
    #[must_use]
    pub const fn required(&self) -> Tag<'tree> {
        self.required
    }

    /// Iterates the chosen descendant up to and including the selected required ancestor.
    pub fn tags(&self) -> impl Iterator<Item = Tag<'tree>> {
        let depth = self.required.depth();
        self.offered
            .ancestors_and_self()
            .take_while(move |tag| tag.depth() >= depth)
    }
}

impl PaintPort {
    /// Reads the port's whitespace-separated offered and required tags.
    #[must_use]
    pub fn new(port_tags: Option<&str>, required_tags: Option<&str>) -> Self {
        Self {
            port_tags: tags(port_tags),
            required_tags: tags(required_tags),
        }
    }

    /// Returns whether a paint and this port satisfy each other's tag requirements.
    ///
    /// The item and vehicle tag slices must contain lowercase ASCII tag names.
    #[must_use]
    pub fn accepts(
        &self,
        item_tags: &[String],
        item_required_tags: &[String],
        vehicle_tags: &[String],
        index: &PaintTagIndex<'_>,
    ) -> bool {
        self.match_sources(item_tags, item_required_tags, vehicle_tags, index, |_| {})
            .is_some()
    }

    /// Returns the actually selected evidence when all authored requirements are satisfied.
    /// The item and vehicle tag slices must contain lowercase ASCII tag names.
    #[must_use]
    pub fn match_with_ancestry<'tree>(
        &self,
        item_tags: &[String],
        item_required_tags: &[String],
        vehicle_tags: &[String],
        index: &PaintTagIndex<'tree>,
    ) -> Option<PaintPortMatch<'tree>> {
        let mut ancestries = Vec::new();
        let sources = self.match_sources(
            item_tags,
            item_required_tags,
            vehicle_tags,
            index,
            |matched| ancestries.push(matched),
        )?;
        Some(PaintPortMatch {
            sources,
            ancestries,
        })
    }

    fn match_sources<'tree>(
        &self,
        item_tags: &[String],
        item_required_tags: &[String],
        vehicle_tags: &[String],
        index: &PaintTagIndex<'tree>,
        mut retain: impl FnMut(PaintTagAncestry<'tree>),
    ) -> Option<PaintOfferSources> {
        if self.required_tags.is_empty()
            || !self
                .required_tags
                .iter()
                .all(|wanted| index.matches(item_tags, wanted, &mut retain))
        {
            return None;
        }
        let mut sources = PaintOfferSources::default();
        for wanted in item_required_tags {
            if index.matches(&self.port_tags, wanted, &mut retain) {
                sources.port = true;
                continue;
            }
            if index.matches(&self.required_tags, wanted, &mut retain) {
                continue;
            }
            if !index.matches(vehicle_tags, wanted, &mut retain) {
                return None;
            }
            sources.vehicle = true;
        }
        Some(sources)
    }
}

fn tags(value: Option<&str>) -> Vec<String> {
    value
        .unwrap_or_default()
        .split_whitespace()
        .map(str::to_ascii_lowercase)
        .collect()
}

/// The tag tree positions indexed by lowercase tag name for paint matching.
#[derive(Debug)]
pub struct PaintTagIndex<'tree> {
    tree: &'tree TagTree,
    positions: HashMap<String, Vec<usize>>,
}

enum Relation<'tree> {
    EqualName,
    Ancestry(PaintTagAncestry<'tree>),
}

impl<'tree> PaintTagIndex<'tree> {
    /// Indexes each tag's original position, including duplicate names.
    #[must_use]
    pub fn new(tree: &'tree TagTree) -> Self {
        let mut positions: HashMap<String, Vec<usize>> = HashMap::new();
        for tag in tree.iter() {
            positions
                .entry(tag.name().to_ascii_lowercase())
                .or_default()
                .push(tag.position());
        }
        Self { tree, positions }
    }

    fn matches(
        &self,
        offered: &[String],
        wanted: &str,
        retain: &mut impl FnMut(PaintTagAncestry<'tree>),
    ) -> bool {
        match offered.iter().find_map(|name| self.relation(name, wanted)) {
            Some(Relation::EqualName) => true,
            Some(Relation::Ancestry(ancestry)) => {
                retain(ancestry);
                true
            }
            None => false,
        }
    }

    fn relation(&self, offered: &str, wanted: &str) -> Option<Relation<'tree>> {
        if offered == wanted {
            return Some(Relation::EqualName);
        }
        let offered_positions = self.positions.get(offered)?;
        let required_positions = self.positions.get(wanted)?;
        for offered in offered_positions.iter().filter_map(|&at| self.tree.at(at)) {
            if let Some(required) = required_positions
                .iter()
                .filter_map(|&at| self.tree.at(at))
                .find(|ancestor| offered.is_within(ancestor))
            {
                return Some(Relation::Ancestry(PaintTagAncestry { offered, required }));
            }
        }
        None
    }
}
