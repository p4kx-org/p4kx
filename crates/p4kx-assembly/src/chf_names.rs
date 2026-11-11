// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};
use std::convert::Infallible;

use p4kx_character::{
    Character, ChfNameKind, Guid as ChfGuid, ItemPort, NameTable, lowered_hash, name_hash,
    skin_attachment,
};
use p4kx_dataforge::Guid;
use p4kx_extraction::{
    InputScope, SourceEvidenceError, SourceInput, SourceLocation, SourceUseRole, SourceUses,
};
use p4kx_model::MaterialLibrary;

use crate::chf_name_sources::{LibraryInput, NameChoice, NameKey, ObservedChfNames};
use crate::{ArchiveModels, ForgeItems, ModelProvider};

const MAX_LIBRARY_LEN: u64 = 16 << 20;

const fn forge_guid(guid: ChfGuid) -> Guid {
    Guid::from_stored(guid.stored())
}

fn items_of(root: &ItemPort) -> Vec<(&ItemPort, String)> {
    let mut seen = HashSet::new();
    let mut items = Vec::new();
    let mut pending = vec![(root, "/character/item_ports".to_owned())];
    while let Some((port, path)) = pending.pop() {
        pending.extend(
            port.children
                .iter()
                .enumerate()
                .map(|(index, child)| (child, format!("{path}/children/{index}"))),
        );
        if seen.insert(port.item) {
            items.push((port, format!("{path}/item")));
        }
    }
    items
}

/// Returns the names supplied by built-in tables and actual worn item and material sources.
pub fn archive_names(
    character: &Character,
    items: &mut ForgeItems<'_, '_>,
    models: &dyn ModelProvider,
) -> NameTable {
    let mut output = Names::new(None);
    let read = |path: &str| {
        let library = models
            .file(path, MAX_LIBRARY_LEN)
            .ok()
            .and_then(|bytes| MaterialLibrary::parse(path, &bytes).ok());
        Ok::<_, Infallible>(library.map(|library| NamedLibrary {
            library,
            input: None,
        }))
    };
    match output.add_inputs(character, items, read) {
        Ok(()) => output.table,
        Err(never) => match never {},
    }
}

/// Returns the exact current name winners and Archive source observations for CHF labels.
///
/// # Errors
/// Returns an error when the shared source-use registry cannot retain an observation.
pub fn archive_names_observed(
    character: &Character,
    items: &mut ForgeItems<'_, '_>,
    models: &ArchiveModels<'_>,
    uses: &SourceUses,
) -> Result<ObservedChfNames, SourceEvidenceError> {
    let mut choices = HashMap::new();
    let mut output = Names::new(Some(&mut choices));
    output.add_inputs(character, items, |path| read_observed(path, models, uses))?;
    let mut names = NameTable::builtin();
    names.merge(&output.table);
    Ok(ObservedChfNames { names, choices })
}

struct Names<'choices> {
    table: NameTable,
    choices: Option<&'choices mut HashMap<NameKey, NameChoice>>,
}

impl<'choices> Names<'choices> {
    fn new(choices: Option<&'choices mut HashMap<NameKey, NameChoice>>) -> Self {
        Self {
            table: NameTable::builtin(),
            choices,
        }
    }

    fn add_inputs<Error>(
        &mut self,
        character: &Character,
        items: &mut ForgeItems<'_, '_>,
        mut read: impl FnMut(&str) -> Result<Option<NamedLibrary>, Error>,
    ) -> Result<(), Error> {
        if let Some(root) = &character.item_ports {
            for (port, selector) in items_of(root) {
                items.visit_port_names(forge_guid(port.item), |record, definition| {
                    self.add_port(&definition.name, || NameChoice::Port {
                        selector: selector.clone(),
                        record,
                        component: definition.component_index,
                        port: definition.port_index,
                    });
                });
            }
        }
        let mut libraries = HashSet::new();
        for (selector, material) in character.materials.iter().enumerate() {
            if material.base_material == ChfGuid::default() {
                continue;
            }
            let Some(target) = items.material_target(forge_guid(material.base_material)) else {
                continue;
            };
            if !libraries.insert(target.path.as_str()) {
                continue;
            }
            if let Some(library) = read(&target.path)? {
                self.add_library(&library, selector, target.record, target.index);
            }
        }
        Ok(())
    }

    fn add_port(&mut self, name: &str, choice: impl Fn() -> NameChoice) {
        let keys = [
            (ChfNameKind::Port, lowered_hash(name)),
            (ChfNameKind::Attachment, skin_attachment(name)),
        ];
        let missing = keys.map(|(kind, hash)| self.table.get(kind, hash).is_none());
        self.table.add_port(name);
        for (key, was_missing) in keys.into_iter().zip(missing) {
            self.retain_choice(key, was_missing, || Some(choice()));
        }
    }

    fn add_library(&mut self, library: &NamedLibrary, selector: usize, record: u32, lookup: usize) {
        for (material, sub) in library.library.materials.iter().enumerate() {
            let choice = |parameter| {
                library.input.as_ref().map(|input| NameChoice::Material {
                    selector,
                    record,
                    lookup,
                    input: input.clone(),
                    material,
                    parameter,
                })
            };
            let kind = ChfNameKind::SubMaterial;
            let hash = lowered_hash(&sub.name);
            let missing = self.table.get(kind, hash).is_none();
            self.table.add_sub_material(&sub.name);
            self.retain_choice((kind, hash), missing, || choice(None));
            for (parameter, (name, _)) in sub.public_params.iter().enumerate() {
                let kind = ChfNameKind::Parameter;
                let hash = name_hash(name);
                let missing = self.table.get(kind, hash).is_none();
                self.table.add_param(name);
                self.retain_choice((kind, hash), missing, || choice(Some(parameter)));
            }
        }
    }

    fn retain_choice(
        &mut self,
        key: NameKey,
        was_missing: bool,
        choice: impl FnOnce() -> Option<NameChoice>,
    ) {
        if was_missing
            && self.table.get(key.0, key.1).is_some()
            && let Some(choices) = self.choices.as_mut()
            && let Some(choice) = choice()
        {
            choices.insert(key, choice);
        }
    }
}

struct NamedLibrary {
    library: MaterialLibrary,
    input: Option<LibraryInput>,
}

fn read_observed(
    path: &str,
    models: &ArchiveModels<'_>,
    uses: &SourceUses,
) -> Result<Option<NamedLibrary>, SourceEvidenceError> {
    let (origin, bytes) = models.observed_file(path, MAX_LIBRARY_LEN);
    let library = bytes
        .ok()
        .and_then(|bytes| MaterialLibrary::parse(path, &bytes).ok());
    let Some(library) = library else {
        if let Some(origin) = origin.as_ref() {
            let scope = uses.observe(SourceInput::Archive(origin), InputScope::Payload)?;
            scope.at(
                SourceLocation::Subresource {
                    namespace: "mtlMaterial".to_owned(),
                    indices: Vec::new(),
                    field: "library".to_owned(),
                },
                SourceUseRole::Attempted,
            )?;
        }
        return Ok(None);
    };
    let input = origin
        .as_ref()
        .map_or(SourceInput::Unknown, SourceInput::Archive);
    let scope = uses.observe(input, InputScope::Payload)?;
    Ok(Some(NamedLibrary {
        library,
        input: Some(LibraryInput {
            scope,
            complete: origin.is_some(),
        }),
    }))
}
