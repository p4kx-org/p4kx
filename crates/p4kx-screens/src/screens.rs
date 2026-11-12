// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::EntityNode;
use p4kx_dataforge::{Guid, Location, StructValue, Value};
use serde::Serialize;

use crate::assets::ScreenAssets;
use crate::canvas::CANVAS_TYPE;
use crate::compose::ScreenComposer;
use crate::inspect::{ElementCount, inspect};

/// One `BuildingBlocks` screen an Entity of a Loadout shows.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Screen {
    /// The path of port names from the root Entity to the Entity that holds the screen, empty
    /// for the root Entity itself.
    pub port: String,
    /// The class name of the Entity that holds the screen.
    pub entity: String,
    /// The record path of that Entity.
    pub entity_record_path: String,
    /// The field of the Entity's Record that refers to the canvas.
    pub field: String,
    /// The GUID of the canvas Record.
    pub canvas_guid: String,
    /// The record path of the canvas Record; `p4kx ui render --screen` takes it.
    pub canvas: String,
    /// The width of the canvas in pixels.
    pub width: f32,
    /// The height of the canvas in pixels.
    pub height: f32,
    /// The UI assets the canvas and the canvases it nests use.
    pub assets: ScreenAssets,
    /// The widgets of the screen by element type.
    pub elements: Vec<ElementCount>,
}

/// A GFx or SWF movie an Entity's UI component names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MovieUse {
    /// The path of port names to the Entity, empty for the root Entity.
    pub port: String,
    /// The class name of the Entity.
    pub entity: String,
    /// The movie path as the Record names it.
    pub movie: String,
}

/// The screens and UI movies of an Entity and everything in its Loadout.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct EntityScreens {
    /// The class name of the root Entity.
    pub entity: String,
    /// The manufacturer code of the Entity, e.g. `aegs`, which `ui render --brand` takes to draw
    /// its screens in the colours of the manufacturer; absent when the Archive has no HUD style
    /// for the prefix of the class name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brand: Option<String>,
    /// The `BuildingBlocks` screens, in Loadout order.
    pub screens: Vec<Screen>,
    /// The UI movies the Entities name.
    pub movies: Vec<MovieUse>,
}

struct Found {
    canvases: Vec<(String, Guid)>,
    movies: Vec<String>,
}

impl ScreenComposer<'_, '_> {
    /// Lists the screens and UI movies of `root` and of every Entity attached in its resolved
    /// Loadout, with the UI assets each screen uses.
    ///
    /// A canvas that cannot be read is kept with an empty size and no assets, so the listing
    /// shows that the Record refers to it.
    pub fn screens_of(&mut self, root: &EntityNode) -> EntityScreens {
        let mut listed = EntityScreens {
            entity: root.class_name().to_owned(),
            brand: self.brand_of(root.class_name()),
            ..EntityScreens::default()
        };
        let mut nodes: Vec<(String, &EntityNode)> = vec![(String::new(), root)];
        for entry in root.loadout() {
            if let Some(node) = entry.entity() {
                nodes.push((entry.path().to_owned(), node));
            }
        }
        for (port, node) in nodes {
            let Some(tree) = self
                .store()
                .forge()
                .record_by_guid(node.guid())
                .and_then(|record| record.tree().ok())
            else {
                continue;
            };
            let found = find(tree.root());
            for movie in found.movies {
                listed.movies.push(MovieUse {
                    port: port.clone(),
                    entity: node.class_name().to_owned(),
                    movie,
                });
            }
            for (field, guid) in found.canvases {
                let screen = self.screen(&port, node, field, guid);
                listed.screens.push(screen);
            }
        }
        listed
    }

    fn screen(&mut self, port: &str, node: &EntityNode, field: String, guid: Guid) -> Screen {
        let canvas = self.store().get(guid);
        let (assets, elements) = canvas.as_ref().map_or_else(
            || (ScreenAssets::default(), Vec::new()),
            |_| inspect(self.store(), guid),
        );
        Screen {
            port: port.to_owned(),
            entity: node.class_name().to_owned(),
            entity_record_path: node.record_path().to_owned(),
            field,
            canvas_guid: guid.to_string(),
            canvas: canvas
                .as_ref()
                .map_or_else(|| guid.to_string(), |canvas| canvas.record_path.clone()),
            width: canvas.as_ref().map_or(0.0, |canvas| canvas.size[0]),
            height: canvas.as_ref().map_or(0.0, |canvas| canvas.size[1]),
            assets,
            elements,
        }
    }
}

fn find(root: &StructValue<'_>) -> Found {
    let mut found = Found {
        canvases: Vec::new(),
        movies: Vec::new(),
    };
    root.visit_owned_values(|visit| match visit.value() {
        Value::Reference(reference) if reference.type_name() == CANVAS_TYPE => {
            found
                .canvases
                .push((Location::new(visit.steps()).to_string(), reference.guid()));
        }
        Value::String(text) if visit.field_name() == "moviePath" && !text.is_empty() => {
            found.movies.push((*text).to_owned());
        }
        _ => {}
    });
    found
}

#[cfg(test)]
mod tests {
    use p4kx_dataforge::DataForge;
    use p4kx_testkit::{
        DataForgeBuilder, TestCanvas, TestInstance, TestRecord, TestStruct, TestType, TestValue,
        entity_schema, screens_schema,
    };

    use super::find;
    use crate::assets::WidgetAssets;

    #[test]
    fn deep_owned_fields_still_supply_screen_references_and_asset_paths()
    -> Result<(), Box<dyn std::error::Error>> {
        const DEPTH: usize = 96;
        const CANVAS: u128 = 0x2a8c_7d22_ee36_478c_8fa2_af44_b63f_c7a3;
        const MOVIE: &str = "Data/UI/synthetic/deep.gfx";
        let mut source = screens_schema(entity_schema(DataForgeBuilder::new()))
            .structure(
                TestStruct::new("Leaf")
                    .field("moviePath", TestType::String)
                    .field("canvas", TestType::Reference),
            )
            .record(TestCanvas::new(CANVAS, "deep", 9.0, 7.0).record());
        let mut nested = TestInstance::new("Leaf")
            .value("moviePath", TestValue::text(MOVIE))
            .value("canvas", TestValue::Reference(CANVAS));
        let mut type_name = "Leaf".to_owned();
        for depth in 0..DEPTH {
            let parent = format!("Owned{depth}");
            source = source
                .structure(TestStruct::new(&parent).field("child", TestType::Class(type_name)));
            nested = TestInstance::new(&parent).value("child", TestValue::Struct(nested));
            type_name = parent;
        }
        let bytes = source
            .structure(TestStruct::new("Owner").field("child", TestType::Class(type_name)))
            .record(
                TestRecord::new(CANVAS + 1, "Owner", "Owner.Sample", "synthetic/owner.xml")
                    .value("child", TestValue::Struct(nested)),
            )
            .build();
        let forge = DataForge::open(&bytes)?;
        let record = forge
            .records_at_path("synthetic/owner")
            .next()
            .ok_or("owner")?;
        let tree = record.tree()?;

        let found = find(tree.root());
        assert_eq!(found.movies, [MOVIE]);
        assert_eq!(
            found.canvases.first().map(|(field, _)| field.as_str()),
            Some(format!("{}canvas", "child.".repeat(DEPTH + 1)).as_str()),
        );
        assert_eq!(found.canvases.len(), 1);
        let assets = WidgetAssets::scan(tree.root());
        assert_eq!(assets.movies, [MOVIE]);
        Ok(())
    }
}
