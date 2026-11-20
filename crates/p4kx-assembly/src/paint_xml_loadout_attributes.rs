// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::ops::ControlFlow;
use std::sync::Mutex;

use p4kx_extraction::{EntryOrigin, SourceLocation};

use crate::{
    LoadoutClassSelection, LoadoutDocumentKind, LoadoutFileObservation, LoadoutReadEvent,
    LoadoutReadObserver, LoadoutSelection, LoadoutXmlClassUse, XmlLoadoutClassSite,
    XmlLoadoutPathSite,
};

#[derive(Debug, Default)]
pub(crate) struct XmlLoadoutAttributes(Mutex<Attributes>);

#[derive(Debug, Default)]
struct Attributes {
    classes: HashMap<LoadoutClassSelection, Attribute>,
    includes: HashMap<LoadoutSelection, Attribute>,
}

#[derive(Debug)]
struct Attribute {
    observation: LoadoutFileObservation,
    location: SourceLocation,
}

impl Attribute {
    fn original(&self) -> Option<(EntryOrigin, SourceLocation)> {
        Some((
            self.observation.source().entry_origin()?.clone(),
            self.location.clone(),
        ))
    }
}

impl XmlLoadoutAttributes {
    pub(super) fn selected(
        &self,
        usage: &LoadoutXmlClassUse,
        selection: &LoadoutClassSelection,
    ) -> Option<Vec<(EntryOrigin, SourceLocation)>> {
        let attributes = self.0.lock().ok()?;
        let getter = attributes.classes.get(selection)?;
        let mut current = usage.original_root();
        let mut selected = Vec::new();
        for (parent, child, path) in usage.includes() {
            let include = attributes.includes.get(path.selection()?)?;
            if !current.is_same_operation(parent) || !parent.is_same_operation(&include.observation)
            {
                return None;
            }
            selected.push(include.original()?);
            current = child;
        }
        if !current.is_same_operation(&getter.observation) {
            return None;
        }
        selected.push(getter.original()?);
        drop(attributes);
        Some(selected)
    }
}

impl LoadoutReadObserver for XmlLoadoutAttributes {
    fn observe(&self, _event: LoadoutReadEvent<'_, '_>) -> ControlFlow<()> {
        ControlFlow::Continue(())
    }

    fn xml_class(
        &self,
        selection: &LoadoutClassSelection,
        site: &XmlLoadoutClassSite<'_>,
    ) -> ControlFlow<()> {
        if site.observation().kind() != LoadoutDocumentKind::XmlLoadout
            || site.attribute_name() != "itemName"
        {
            return ControlFlow::Continue(());
        }
        let Some(location) = site.location() else {
            return ControlFlow::Continue(());
        };
        let Ok(mut attributes) = self.0.lock() else {
            return ControlFlow::Break(());
        };
        attributes.classes.insert(
            selection.clone(),
            Attribute {
                observation: site.observation().clone(),
                location,
            },
        );
        ControlFlow::Continue(())
    }

    fn xml_path(
        &self,
        selection: &LoadoutSelection,
        site: &XmlLoadoutPathSite<'_>,
    ) -> ControlFlow<()> {
        if site.observation().kind() != LoadoutDocumentKind::XmlLoadout
            || site.attribute_name() != "loadout"
        {
            return ControlFlow::Continue(());
        }
        let Some(location) = site.location() else {
            return ControlFlow::Continue(());
        };
        let Ok(mut attributes) = self.0.lock() else {
            return ControlFlow::Break(());
        };
        attributes.includes.insert(
            selection.clone(),
            Attribute {
                observation: site.observation().clone(),
                location,
            },
        );
        ControlFlow::Continue(())
    }
}
