// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_dataforge::{StructValue, Value};
use serde::Serialize;

use crate::screen_asset_inputs::{ScreenAssetInputs, WidgetAssetInput, WidgetInput};

#[derive(Debug, Clone)]
pub(crate) struct WidgetAsset {
    pub(crate) value: String,
    ordinal: usize,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct WidgetAssets {
    pub(crate) textures: Vec<WidgetAsset>,
    pub(crate) vectors: Vec<WidgetAsset>,
    pub(crate) movies: Vec<WidgetAsset>,
    pub(crate) keys: Vec<WidgetAsset>,
}

impl WidgetAssets {
    pub(crate) fn scan(structure: &StructValue<'_>) -> Self {
        let mut assets = Self::default();
        structure.visit_owned_values(|visit| match visit.value() {
            Value::String(text) => assets.classify(text, visit.ordinal()),
            Value::Locale(key) => assets.key(key, visit.ordinal()),
            _ => {}
        });
        assets
    }

    fn key(&mut self, key: &str, ordinal: usize) {
        let key = key.trim_start_matches('@');
        if !key.is_empty() && !matches!(key, "LOC_EMPTY" | "LOC_UNINITIALIZED" | "LOC_PLACEHOLDER")
        {
            self.keys.push(WidgetAsset {
                value: key.to_owned(),
                ordinal,
            });
        }
    }

    fn classify(&mut self, text: &str, ordinal: usize) {
        if let Some(key) = text.strip_prefix('@') {
            self.key(key, ordinal);
            return;
        }
        let lower = text.to_ascii_lowercase();
        let extension = lower.rsplit_once('.').map(|(_, extension)| extension);
        let original = || WidgetAsset {
            value: text.to_owned(),
            ordinal,
        };
        match extension {
            Some("tif" | "tiff") => {
                if let Some((base, _)) = text.rsplit_once('.') {
                    self.textures.push(WidgetAsset {
                        value: format!("{base}.dds"),
                        ordinal,
                    });
                }
            }
            Some("dds" | "png" | "jpg" | "tga") => self.textures.push(original()),
            Some("svg") => self.vectors.push(original()),
            Some("gfx" | "swf") => self.movies.push(original()),
            _ => {}
        }
    }
}

/// The UI assets a screen draws on, from the canvas Records it is made of.
///
/// Paths are as the Records name them, except that `.tif` textures are listed as the `.dds`
/// files the Archive holds. Every list is sorted and holds each entry once.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ScreenAssets {
    /// The record paths of the canvases the screen nests, its own canvas first.
    pub canvases: Vec<String>,
    /// The textures widgets show.
    pub textures: Vec<String>,
    /// The vector graphics (`.svg`) widgets show.
    pub vectors: Vec<String>,
    /// The GFx and SWF movies widgets name.
    pub movies: Vec<String>,
    /// The localisation keys of texts, without the leading `@`.
    pub localisation_keys: Vec<String>,
}

#[derive(Debug)]
struct ListedCanvas {
    path: String,
    record_index: u32,
}

#[derive(Debug, Default)]
pub(crate) struct AssetSet {
    canvases: Vec<ListedCanvas>,
    textures: BTreeMap<String, WidgetAssetInput>,
    vectors: BTreeMap<String, WidgetAssetInput>,
    movies: BTreeMap<String, WidgetAssetInput>,
    keys: BTreeMap<String, WidgetAssetInput>,
}

impl AssetSet {
    pub(crate) fn add_canvas(&mut self, record_path: &str, record_index: u32) {
        if !self
            .canvases
            .iter()
            .any(|canvas| canvas.path == record_path)
        {
            self.canvases.push(ListedCanvas {
                path: record_path.to_owned(),
                record_index,
            });
        }
    }

    pub(crate) fn add(&mut self, assets: &WidgetAssets, widget: WidgetInput) {
        for (target, values) in [
            (&mut self.textures, &assets.textures),
            (&mut self.vectors, &assets.vectors),
            (&mut self.movies, &assets.movies),
            (&mut self.keys, &assets.keys),
        ] {
            for asset in values {
                target
                    .entry(asset.value.clone())
                    .or_insert(WidgetAssetInput {
                        widget,
                        ordinal: asset.ordinal,
                    });
            }
        }
    }

    pub(crate) fn finish(self, mut inputs: Option<&mut ScreenAssetInputs>) -> ScreenAssets {
        let canvases = self
            .canvases
            .into_iter()
            .map(|canvas| {
                if let Some(inputs) = inputs.as_deref_mut() {
                    inputs.canvases.push(canvas.record_index);
                }
                canvas.path
            })
            .collect();
        ScreenAssets {
            canvases,
            textures: group(
                self.textures,
                inputs.as_deref_mut().map(|inputs| &mut inputs.textures),
            ),
            vectors: group(
                self.vectors,
                inputs.as_deref_mut().map(|inputs| &mut inputs.vectors),
            ),
            movies: group(
                self.movies,
                inputs.as_deref_mut().map(|inputs| &mut inputs.movies),
            ),
            localisation_keys: group(
                self.keys,
                inputs.map(|inputs| &mut inputs.localisation_keys),
            ),
        }
    }
}

fn group(
    sorted: BTreeMap<String, WidgetAssetInput>,
    inputs: Option<&mut Vec<WidgetAssetInput>>,
) -> Vec<String> {
    if let Some(inputs) = inputs {
        sorted
            .into_iter()
            .map(|(value, input)| {
                inputs.push(input);
                value
            })
            .collect()
    } else {
        sorted.into_keys().collect()
    }
}
