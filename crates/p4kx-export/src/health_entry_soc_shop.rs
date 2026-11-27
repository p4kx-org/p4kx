// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::cryxml::{SourceAttribute, SourceElement};
use p4kx_extraction::{SourceLocation, SourceUseRole};

use super::super::entry_selection_input::{EntrySelectionUse, SelectionSource};
use super::super::{AuthoredEntryPath, EntryCapture};
use super::{EntrySocEntity, ordinal};
use crate::CatalogError;

const SHOP: &str = "EntityComponentShop";
const INVENTORY: &str = "offlineInventory";
const PARAMETERS: &str = "SShopComponentParams";

pub(crate) enum ShopDeclarations<'source> {
    Absent,
    Unattributed,
    Authored {
        guid: &'source str,
        markers: Vec<AuthoredEntryPath>,
        values: Vec<AuthoredEntryPath>,
        unsupported: bool,
    },
}

impl<'source> EntrySocEntity<'source> {
    pub(crate) fn shop_declarations(
        &self,
        capture: &EntryCapture<'_>,
    ) -> Result<ShopDeclarations<'source>, CatalogError> {
        self.origin.check_frame(&capture.frame())?;
        let entity = self.entity.element();
        let mut components = components(entity).peekable();
        if components.peek().is_none() {
            return Ok(if contains_shop(entity) {
                ShopDeclarations::Unattributed
            } else {
                ShopDeclarations::Absent
            });
        }
        let mut guids = self.entity.guid_attributes();
        let Some(guid) = guids.next().filter(|_| guids.next().is_none()) else {
            return Ok(ShopDeclarations::Unattributed);
        };
        if entity.namespace().is_some() {
            return Ok(ShopDeclarations::Unattributed);
        }
        let sites = Sites {
            owner: self,
            capture,
            chunk: ordinal(self.index)?,
        };
        let initial_selectors = [sites.attribute(&guid)?, sites.node(entity)?];
        let mut markers = Vec::new();
        let mut values = Vec::new();
        let mut supported = Vec::new();
        let mut unsupported = false;
        for (wrapper, component) in components {
            supported.push(component.position());
            let (marker, unknown) =
                sites.component(wrapper, component, &initial_selectors, &mut values)?;
            markers.push(marker);
            unsupported |= unknown;
        }
        unsupported |= descendants(entity)
            .filter(|node| node.name().eq_ignore_ascii_case(SHOP))
            .any(|node| !supported.contains(&node.position()));
        Ok(ShopDeclarations::Authored {
            guid: guid.value(),
            markers,
            values,
            unsupported,
        })
    }
}

struct Sites<'entity, 'source, 'capture, 'archive> {
    owner: &'entity EntrySocEntity<'source>,
    capture: &'capture EntryCapture<'archive>,
    chunk: u32,
}

impl Sites<'_, '_, '_, '_> {
    fn component(
        &self,
        wrapper: Option<SourceElement<'_>>,
        component: SourceElement<'_>,
        initial_selectors: &[SourceLocation],
        values: &mut Vec<AuthoredEntryPath>,
    ) -> Result<(AuthoredEntryPath, bool), CatalogError> {
        let mut selectors = initial_selectors.to_vec();
        if let Some(wrapper) = wrapper {
            selectors.push(self.node(wrapper)?);
        }
        let location = self.node(component)?;
        let marker = self.retain(component.name(), location.clone(), &selectors)?;
        selectors.push(location);
        let mut unsupported = self.values(component, &selectors, values)?;
        for child in component.elements() {
            if named(child, PARAMETERS) {
                let mut parameter_selectors = selectors.clone();
                parameter_selectors.push(self.node(child)?);
                unsupported |= self.values(child, &parameter_selectors, values)?;
                unsupported |= child.elements().any(contains_inventory);
            } else {
                unsupported |= contains_inventory(child);
            }
        }
        Ok((marker, unsupported))
    }

    fn values(
        &self,
        node: SourceElement<'_>,
        selectors: &[SourceLocation],
        output: &mut Vec<AuthoredEntryPath>,
    ) -> Result<bool, CatalogError> {
        let mut unsupported = false;
        for attribute in node
            .attributes()
            .filter(|attribute| inventory_field(attribute.name()))
        {
            if attribute.namespace().is_some() {
                unsupported = true;
                continue;
            }
            let location = self.attribute(&attribute)?;
            let mut field_selectors = selectors.to_vec();
            field_selectors.push(location.clone());
            output.push(self.retain(attribute.value(), location, &field_selectors)?);
        }
        Ok(unsupported)
    }

    fn retain(
        &self,
        value: &str,
        payload: SourceLocation,
        selectors: &[SourceLocation],
    ) -> Result<AuthoredEntryPath, CatalogError> {
        let input = |location, role| EntrySelectionUse {
            source: SelectionSource::Entry(self.owner.origin.clone()),
            location,
            role,
        };
        let mut uses = Vec::with_capacity(1 + selectors.len());
        uses.push(input(payload, SourceUseRole::Payload));
        uses.extend(
            selectors
                .iter()
                .cloned()
                .map(|location| input(location, SourceUseRole::Selector)),
        );
        AuthoredEntryPath::captured(self.capture, value, uses)
    }

    fn node(&self, node: SourceElement<'_>) -> Result<SourceLocation, CatalogError> {
        self.owner.element_location(self.chunk, node)
    }

    fn attribute(&self, attribute: &SourceAttribute<'_>) -> Result<SourceLocation, CatalogError> {
        self.owner
            .attribute_location(self.chunk, attribute.name(), attribute.position())
    }
}

fn components(
    entity: SourceElement<'_>,
) -> impl Iterator<Item = (Option<SourceElement<'_>>, SourceElement<'_>)> {
    entity.elements().flat_map(|child| {
        let direct = named(child, SHOP).then_some((None, child));
        let wrapper = ["PropertiesDataCore", "Components"]
            .into_iter()
            .any(|name| named(child, name))
            .then_some(child);
        direct
            .into_iter()
            .chain(wrapper.into_iter().flat_map(|wrapper| {
                wrapper
                    .elements()
                    .filter(|node| named(*node, SHOP))
                    .map(move |node| (Some(wrapper), node))
            }))
    })
}

fn descendants(root: SourceElement<'_>) -> impl Iterator<Item = SourceElement<'_>> {
    let mut pending = vec![root];
    std::iter::from_fn(move || {
        let node = pending.pop()?;
        pending.extend(node.elements());
        Some(node)
    })
}

fn contains_shop(root: SourceElement<'_>) -> bool {
    descendants(root).any(|node| node.name().eq_ignore_ascii_case(SHOP))
}

fn contains_inventory(root: SourceElement<'_>) -> bool {
    descendants(root).any(|node| {
        inventory_field(node.name())
            || node
                .attributes()
                .any(|attribute| inventory_field(attribute.name()))
    })
}

fn inventory_field(field: &str) -> bool {
    field
        .get(..INVENTORY.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(INVENTORY))
}

fn named(element: SourceElement<'_>, name: &str) -> bool {
    element.namespace().is_none() && element.name().eq_ignore_ascii_case(name)
}
