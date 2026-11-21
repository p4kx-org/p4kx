// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_dataforge::TagTree;

/// The offered and required tags of one typed paint port.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct PaintPort {
    port_tags: Vec<String>,
    required_tags: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct PaintOfferSources {
    pub(crate) port: bool,
    pub(crate) vehicle: bool,
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
        self.accept_with_sources(item_tags, item_required_tags, vehicle_tags, index)
            .is_some()
    }

    pub(crate) fn accept_with_sources(
        &self,
        item_tags: &[String],
        item_required_tags: &[String],
        vehicle_tags: &[String],
        index: &PaintTagIndex<'_>,
    ) -> Option<PaintOfferSources> {
        if self.required_tags.is_empty()
            || !self.required_tags.iter().all(|wanted| {
                item_tags
                    .iter()
                    .any(|offered| index.matches(offered, wanted))
            })
        {
            return None;
        }
        let mut sources = PaintOfferSources::default();
        for wanted in item_required_tags {
            let matches = |offered: &String| index.matches(offered, wanted);
            if self.port_tags.iter().any(matches) {
                sources.port = true;
                continue;
            }
            if self.required_tags.iter().any(matches) {
                continue;
            }
            if !vehicle_tags.iter().any(matches) {
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

    fn matches(&self, offered: &str, wanted: &str) -> bool {
        if offered == wanted {
            return true;
        }
        let (Some(below), Some(above)) = (self.positions.get(offered), self.positions.get(wanted))
        else {
            return false;
        };
        below.iter().filter_map(|&at| self.tree.at(at)).any(|tag| {
            above
                .iter()
                .filter_map(|&at| self.tree.at(at))
                .any(|ancestor| tag.is_within(&ancestor))
        })
    }
}
