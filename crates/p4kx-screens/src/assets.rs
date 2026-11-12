// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_dataforge::{StructValue, Value};
use serde::Serialize;

#[derive(Debug, Clone, Default)]
pub(crate) struct WidgetAssets {
    pub(crate) textures: Vec<String>,
    pub(crate) vectors: Vec<String>,
    pub(crate) movies: Vec<String>,
    pub(crate) keys: Vec<String>,
}

impl WidgetAssets {
    pub(crate) fn scan(structure: &StructValue<'_>) -> Self {
        let mut assets = Self::default();
        structure.visit_owned_values(|visit| match visit.value() {
            Value::String(text) => assets.classify(text),
            Value::Locale(key) => assets.key(key),
            _ => {}
        });
        assets
    }

    fn key(&mut self, key: &str) {
        let key = key.trim_start_matches('@');
        if !key.is_empty() && !matches!(key, "LOC_EMPTY" | "LOC_UNINITIALIZED" | "LOC_PLACEHOLDER")
        {
            self.keys.push(key.to_owned());
        }
    }

    fn classify(&mut self, text: &str) {
        if let Some(key) = text.strip_prefix('@') {
            self.key(key);
            return;
        }
        let lower = text.to_ascii_lowercase();
        let extension = lower.rsplit_once('.').map(|(_, extension)| extension);
        match extension {
            Some("tif" | "tiff") => {
                if let Some((base, _)) = text.rsplit_once('.') {
                    self.textures.push(format!("{base}.dds"));
                }
            }
            Some("dds" | "png" | "jpg" | "tga") => self.textures.push(text.to_owned()),
            Some("svg") => self.vectors.push(text.to_owned()),
            Some("gfx" | "swf") => self.movies.push(text.to_owned()),
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

#[derive(Debug, Default)]
pub(crate) struct AssetSet {
    pub(crate) canvases: Vec<String>,
    textures: BTreeSet<String>,
    vectors: BTreeSet<String>,
    movies: BTreeSet<String>,
    keys: BTreeSet<String>,
}

impl AssetSet {
    pub(crate) fn add_canvas(&mut self, record_path: &str) {
        if !self.canvases.iter().any(|path| path == record_path) {
            self.canvases.push(record_path.to_owned());
        }
    }

    pub(crate) fn add(&mut self, assets: &WidgetAssets) {
        self.textures.extend(assets.textures.iter().cloned());
        self.vectors.extend(assets.vectors.iter().cloned());
        self.movies.extend(assets.movies.iter().cloned());
        self.keys.extend(assets.keys.iter().cloned());
    }

    pub(crate) fn finish(self) -> ScreenAssets {
        ScreenAssets {
            canvases: self.canvases,
            textures: self.textures.into_iter().collect(),
            vectors: self.vectors.into_iter().collect(),
            movies: self.movies.into_iter().collect(),
            localisation_keys: self.keys.into_iter().collect(),
        }
    }
}
