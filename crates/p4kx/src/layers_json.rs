// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The unresolved layer files in the JSON report of an Assembly export.

use p4kx_extraction::model::UnresolvedLayers;
use serde::Serialize;

/// The layer files the material libraries of an export could not resolve, flattened into its
/// report as `unresolved_layers` and, when there is one, `first_unresolved_layer`.
#[derive(Serialize)]
pub(crate) struct LayersJson {
    unresolved_layers: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    first_unresolved_layer: Option<String>,
}

impl From<&UnresolvedLayers> for LayersJson {
    fn from(layers: &UnresolvedLayers) -> Self {
        Self {
            unresolved_layers: layers.count(),
            first_unresolved_layer: layers.first().map(str::to_owned),
        }
    }
}

#[cfg(test)]
mod tests {
    use p4kx_extraction::model::{MaterialLibrary, MaterialStatus};

    use super::{LayersJson, UnresolvedLayers};

    fn status(unresolved: &[&str]) -> Result<MaterialStatus, Box<dyn std::error::Error>> {
        let mut library = MaterialLibrary::parse("a", b"<Material/>")?;
        library.unresolved_layers = unresolved.iter().map(|path| (*path).to_owned()).collect();
        Ok(MaterialStatus::Loaded(library))
    }

    #[test]
    fn a_file_named_by_two_libraries_counts_once_and_the_first_is_kept()
    -> Result<(), Box<dyn std::error::Error>> {
        let statuses = [status(&["b.mtl", "a.mtl"])?, status(&["a.mtl", "c.mtl"])?];

        let json = serde_json::to_value(LayersJson::from(&UnresolvedLayers::of(&statuses)))?;

        assert_eq!(
            json,
            serde_json::json!({"unresolved_layers": 3, "first_unresolved_layer": "b.mtl"})
        );
        Ok(())
    }

    #[test]
    fn nothing_unresolved_has_no_first_path() -> Result<(), Box<dyn std::error::Error>> {
        let json = serde_json::to_value(LayersJson::from(&UnresolvedLayers::default()))?;

        assert_eq!(json, serde_json::json!({"unresolved_layers": 0}));
        Ok(())
    }
}
