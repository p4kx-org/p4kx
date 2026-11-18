// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::sync::Arc;

use p4kx_assembly::{ModelFailure, ModelProvider, ProvidedModel};
use p4kx_extraction::{EntryOrigin, PathIndex};
use p4kx_model::{
    CompanionError, CompanionPath, LayerBudget, LayerCache, MaterialLibrary, MaterialReading,
    MaterialStatus,
};

use super::input::{Request, canonical};
use super::{Check, sources};

pub(super) struct Loaded {
    pub(super) status: MaterialStatus,
    pub(super) original: EntryOrigin,
    layers: BTreeMap<String, (Arc<MaterialStatus>, EntryOrigin)>,
}

impl Loaded {
    pub(super) fn read(
        index: &PathIndex<'_>,
        request: &Request,
        ledger: &mut sources::Ledger,
    ) -> Check<Self> {
        let LayerFiles { raw, layers } = read_layers(index, request, ledger)?;
        let bytes = ledger.read(index, &request.wrapper, sources::ReadRole::WrapperMaterial)?;
        let mut resolver = |path: CompanionPath<'_>| -> Result<Option<Vec<u8>>, CompanionError> {
            Ok(match path {
                CompanionPath::Material(name) => raw.get(&material_key(name)).cloned(),
                _ => None,
            })
        };
        let library = MaterialLibrary::parse_with_resolver(
            &request.wrapper.path,
            &bytes,
            Some(request.wrapper.path.clone()),
            &mut resolver,
            MaterialReading::Drawn,
            &LayerCache::new(),
        )?;
        check_wrapper(&library, request)?;
        let original = index
            .archive()
            .entry_origin(sources::entry(index, &request.wrapper)?)
            .ok_or("the original wrapper Entry proof")?;
        Ok(Self {
            status: MaterialStatus::Loaded(library),
            original,
            layers,
        })
    }
}

struct LayerFiles {
    raw: BTreeMap<String, Vec<u8>>,
    layers: BTreeMap<String, (Arc<MaterialStatus>, EntryOrigin)>,
}

fn read_layers(
    index: &PathIndex<'_>,
    request: &Request,
    ledger: &mut sources::Ledger,
) -> Check<LayerFiles> {
    let mut raw = BTreeMap::new();
    let mut layers = BTreeMap::new();
    for layer in &request.layers {
        let bytes = ledger.read(index, &layer.entry, sources::ReadRole::LayerMaterial)?;
        let library = MaterialLibrary::parse(&layer.entry.path, &bytes)?;
        let selected = library
            .materials
            .get(layer.source_ordinal)
            .ok_or("bound child ordinal")?;
        if !selected.layers.is_empty() {
            return Err("the child now has a deeper layer closure".into());
        }
        let origin = index
            .archive()
            .entry_origin(sources::entry(index, &layer.entry)?)
            .ok_or("the original child Entry proof")?;
        raw.insert(material_key(&layer.path), bytes);
        layers.insert(
            material_key(&layer.path),
            (Arc::new(MaterialStatus::Loaded(library)), origin),
        );
    }
    Ok(LayerFiles { raw, layers })
}

fn check_wrapper(library: &MaterialLibrary, request: &Request) -> Check {
    let wrapper = library
        .materials
        .get(request.material_ordinal)
        .ok_or("bound wrapper slot")?;
    if wrapper.name != request.material_name {
        return Err("the original wrapper name differs".into());
    }
    if wrapper.shader != request.shader
        || wrapper.string_gen_mask != request.string_gen_mask
        || wrapper.public_params != request.public_params
        || wrapper
            .layers
            .iter()
            .map(|row| &row.attributes)
            .ne(request.layer_attributes.iter())
        || wrapper.layers.iter().any(|row| row.surface.is_none())
        || wrapper.wear_blend() != 0.0
    {
        return Err("the selected original wrapper differs or its layers are unresolved".into());
    }
    Ok(())
}

fn material_key(path: &str) -> String {
    canonical(path)
        .trim_start_matches("data/")
        .trim_end_matches(".mtl")
        .to_owned()
}

impl ModelProvider for Loaded {
    fn model(&self, _path: &str, _lod: u32) -> Result<ProvidedModel, ModelFailure> {
        Err(ModelFailure::Missing)
    }

    fn library(&self, _entry: &str, path: &str) -> MaterialStatus {
        self.layers.get(&material_key(path)).map_or_else(
            || MaterialStatus::Missing {
                path: path.to_owned(),
                target: None,
            },
            |(status, _)| status.as_ref().clone(),
        )
    }

    fn layer_library_observed(
        &self,
        path: &str,
        _budget: &mut LayerBudget,
    ) -> (Arc<MaterialStatus>, Option<EntryOrigin>) {
        self.layers.get(&material_key(path)).map_or_else(
            || {
                (
                    Arc::new(MaterialStatus::Missing {
                        path: path.to_owned(),
                        target: None,
                    }),
                    None,
                )
            },
            |(status, origin)| (Arc::clone(status), Some(origin.clone())),
        )
    }
}
