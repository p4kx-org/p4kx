// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Guid, SourceUse};

use super::{CONFIG_ATTRIBUTE, CONTAINER_ATTRIBUTE};
use crate::CatalogError;
use crate::catalog_read::original::{AuthoredEntryPath, EntryXml};
use crate::starmap_index::Index;
use crate::starmap_xml::{Node, Root};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Scope {
    Entity,
    ObjectContainerComponent,
    Other,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct Shape {
    owner: Scope,
    namespaced: bool,
    selected_attribute_occurrences: usize,
    populated_selectors: usize,
    resolved_alternatives_on_same_entity: usize,
    valid_config_on_selected_node: usize,
    valid_config_on_same_entity: usize,
    valid_config_outside_or_unscoped: usize,
    malformed_config: usize,
}

#[derive(Clone, Copy)]
struct Site {
    index: u32,
    entity: Option<u32>,
}

struct Selected {
    site: Site,
    scope: Scope,
    attributes: usize,
    namespaced: bool,
}

#[derive(Default)]
struct Sites {
    selected: Option<Selected>,
    alternatives: Vec<Site>,
    configs: Vec<Site>,
    selector_count: usize,
    malformed_config: usize,
}

impl Sites {
    fn add(
        &mut self,
        index: &Index<'_>,
        xml: &EntryXml<'_, '_, '_>,
        node: Root<'_>,
        site: Site,
        admitted: &AuthoredEntryPath,
        selected_uses: &[SourceUse],
    ) -> Result<(), CatalogError> {
        self.add_configs(node, site);
        let selectors = node.attributes().filter(|(field, value)| {
            field.eq_ignore_ascii_case(CONTAINER_ATTRIBUTE) && !value.is_empty()
        });
        let mut count = 0;
        for (_, value) in selectors {
            count += 1;
            if value != admitted.as_str() && index.candidates(value).len() == 1 {
                self.alternatives.push(site);
            }
        }
        self.selector_count += count;
        if matches_selected_attribute(xml, node, count, selected_uses)? {
            if self.selected.is_some() {
                return Err(crate::ExportAccountingError::ConflictingSourceDatum.into());
            }
            self.selected = Some(Selected {
                site,
                scope: scope(node),
                attributes: count,
                namespaced: namespaced(node),
            });
        }
        Ok(())
    }

    fn add_configs(&mut self, node: Root<'_>, site: Site) {
        let configs = node.attributes().filter(|(field, value)| {
            field.eq_ignore_ascii_case(CONFIG_ATTRIBUTE) && !value.trim().is_empty()
        });
        for (_, value) in configs {
            if value.trim().parse::<Guid>().is_ok() {
                self.configs.push(site);
            } else {
                self.malformed_config += 1;
            }
        }
    }

    fn finish(self) -> Option<Shape> {
        let selected = self.selected?;
        let same_entity =
            |site: &Site| selected.site.entity.is_some() && selected.site.entity == site.entity;
        Some(Shape {
            owner: selected.scope,
            namespaced: selected.namespaced,
            selected_attribute_occurrences: selected.attributes,
            populated_selectors: self.selector_count,
            resolved_alternatives_on_same_entity: self
                .alternatives
                .iter()
                .filter(|site| same_entity(site))
                .count(),
            valid_config_on_selected_node: self
                .configs
                .iter()
                .filter(|site| site.index == selected.site.index)
                .count(),
            valid_config_on_same_entity: self
                .configs
                .iter()
                .filter(|site| same_entity(site))
                .count(),
            valid_config_outside_or_unscoped: self
                .configs
                .iter()
                .filter(|site| site.index != selected.site.index && !same_entity(site))
                .count(),
            malformed_config: self.malformed_config,
        })
    }
}

pub(super) fn inspect(
    index: &Index<'_>,
    xml: &EntryXml<'_, '_, '_>,
    admitted: &AuthoredEntryPath,
) -> Result<Option<Shape>, CatalogError> {
    let (selected_uses, complete) = admitted.selector_inputs()?;
    if !complete || selected_uses.is_empty() {
        return Ok(None);
    }
    let mut sites = Sites::default();
    let mut pending = vec![(xml.root(), None)];
    while let Some((node, entity)) = pending.pop() {
        let index_in_xml = xml.node_source(node)?.index();
        let entity = if scope(node) == Scope::Entity && !namespaced(node) {
            Some(index_in_xml)
        } else {
            entity
        };
        let site = Site {
            index: index_in_xml,
            entity,
        };
        sites.add(index, xml, node, site, admitted, &selected_uses)?;
        pending.extend(node.children().map(|child| (child, entity)));
    }
    Ok(sites.finish())
}

fn matches_selected_attribute(
    xml: &EntryXml<'_, '_, '_>,
    node: Root<'_>,
    populated_selectors: usize,
    selected_uses: &[SourceUse],
) -> Result<bool, CatalogError> {
    if populated_selectors == 0 {
        return Ok(false);
    }
    let Some(candidate) = xml.attribute(node, CONTAINER_ATTRIBUTE)? else {
        return Ok(false);
    };
    let (candidate_uses, complete) = candidate.selector_inputs()?;
    Ok(complete
        && candidate_uses
            .iter()
            .map(SourceUse::reference)
            .eq(selected_uses.iter().map(SourceUse::reference)))
}

fn scope(node: Root<'_>) -> Scope {
    if node.name().eq_ignore_ascii_case("Entity") {
        Scope::Entity
    } else if node
        .name()
        .eq_ignore_ascii_case("EntityComponentObjectContainer")
    {
        Scope::ObjectContainerComponent
    } else {
        Scope::Other
    }
}

fn namespaced(node: Root<'_>) -> bool {
    match node {
        Root::Text(node) => {
            node.tag_name().namespace().is_some()
                || roxmltree::Node::attributes(&node).any(|attribute| {
                    attribute.namespace().is_some()
                        && attribute.name().eq_ignore_ascii_case(CONTAINER_ATTRIBUTE)
                })
        }
        Root::Cry(_) => false,
    }
}
