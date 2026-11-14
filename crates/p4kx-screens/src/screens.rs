// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{Attachment, EntityNode, PortNode};
use p4kx_dataforge::{Guid, Location, StructValue, Value};
use serde::Serialize;

use crate::assets::ScreenAssets;
use crate::canvas::CANVAS_TYPE;
use crate::compose::ScreenComposer;
use crate::inspect::{ElementCount, inspect};
use crate::screen_asset_inputs::ScreenAssetInputs;
use crate::screen_listing_inputs::{
    AttachmentInput, CanvasInput, EntityScreenInputs, MovieInput, ObservedEntityScreens,
    ScreenInput, ValueInput,
};

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
    canvases: Vec<FoundCanvas>,
    movies: Vec<FoundMovie>,
}

struct FoundCanvas {
    field: String,
    guid: Guid,
    ordinal: usize,
    target_record: u32,
    stored_guid: bool,
}

struct FoundMovie {
    path: String,
    ordinal: usize,
}

impl ScreenComposer<'_, '_> {
    /// Lists the screens and UI movies of `root` and of every Entity attached in its resolved
    /// Loadout, with the UI assets each screen uses.
    ///
    /// A canvas that cannot be read is kept with an empty size and no assets, so the listing
    /// shows that the Record refers to it.
    pub fn screens_of(&mut self, root: &EntityNode) -> EntityScreens {
        self.list_screens(root, None)
    }

    /// Lists the same rows and retains the original logical inputs selected during the read.
    pub fn screens_of_observed(&mut self, root: &EntityNode) -> ObservedEntityScreens {
        let mut inputs = EntityScreenInputs::default();
        let listing = self.list_screens(root, Some(&mut inputs));
        ObservedEntityScreens { listing, inputs }
    }

    fn list_screens(
        &mut self,
        root: &EntityNode,
        mut inputs: Option<&mut EntityScreenInputs>,
    ) -> EntityScreens {
        let brand = self.brand_selection(root.class_name());
        if let Some(inputs) = inputs.as_deref_mut() {
            inputs.entity_record = root.record_index();
            inputs.brand_style_record = brand.as_ref().map(|brand| brand.record_index);
        }
        let mut listed = EntityScreens {
            entity: root.class_name().to_owned(),
            brand: brand.map(|brand| brand.code),
            ..EntityScreens::default()
        };
        self.list_entity("", root, None, &mut listed, inputs.as_deref_mut());
        root.visit_ports(|path, route| {
            if let Some(port) = route.last()
                && let Attachment::Entity(child) = port.attachment()
            {
                self.list_entity(path, child, Some(route), &mut listed, inputs.as_deref_mut());
            }
        });
        listed
    }

    fn list_entity(
        &mut self,
        port: &str,
        node: &EntityNode,
        route: Option<&[&PortNode]>,
        listed: &mut EntityScreens,
        mut inputs: Option<&mut EntityScreenInputs>,
    ) {
        let Some(tree) = self
            .store()
            .forge()
            .record_by_guid(node.guid())
            .and_then(|record| record.tree().ok())
        else {
            return;
        };
        let found = find(tree.root());
        if found.canvases.is_empty() && found.movies.is_empty() {
            return;
        }
        let attachment_route = inputs.as_deref_mut().zip(route).map(|(inputs, route)| {
            let index = inputs.attachment_routes.len();
            inputs.attachment_routes.push(AttachmentInput::read(route));
            index
        });
        let source = |ordinal| ValueInput {
            record_index: tree.record_index(),
            ordinal,
        };
        for movie in found.movies {
            if let Some(inputs) = inputs.as_deref_mut() {
                inputs.movies.push(MovieInput {
                    entity_record: node.record_index(),
                    movie_path: source(movie.ordinal),
                    attachment_route,
                });
            }
            listed.movies.push(MovieUse {
                port: port.to_owned(),
                entity: node.class_name().to_owned(),
                movie: movie.path,
            });
        }
        for canvas in found.canvases {
            let reference = source(canvas.ordinal);
            let target_record = canvas.target_record;
            let stored_guid = canvas.stored_guid;
            let mut assets = ScreenAssetInputs::default();
            let observed = inputs.is_some().then_some(&mut assets);
            let (screen, canvas) = self.screen(port, node, canvas, observed);
            if let Some(inputs) = inputs.as_deref_mut() {
                inputs.screens.push(ScreenInput {
                    entity_record: node.record_index(),
                    reference,
                    target_record,
                    stored_guid,
                    canvas,
                    assets,
                    attachment_route,
                });
            }
            listed.screens.push(screen);
        }
    }

    fn screen(
        &mut self,
        port: &str,
        node: &EntityNode,
        FoundCanvas {
            field,
            guid,
            target_record,
            ..
        }: FoundCanvas,
        inputs: Option<&mut ScreenAssetInputs>,
    ) -> (Screen, CanvasInput) {
        let canvas = self.store().get(guid);
        let (assets, elements) = canvas.as_ref().map_or_else(
            || (ScreenAssets::default(), Vec::new()),
            |_| inspect(self.store(), guid, inputs),
        );
        let listed = Screen {
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
        };
        let input = match canvas {
            Some(canvas) if canvas.record_index == target_record => CanvasInput::Read {
                record_index: canvas.record_index,
                width_read: canvas.size_fields[0],
                height_read: canvas.size_fields[1],
            },
            Some(_) => CanvasInput::Unproved,
            None => CanvasInput::Unreadable,
        };
        (listed, input)
    }
}

fn find(root: &StructValue<'_>) -> Found {
    let mut found = Found {
        canvases: Vec::new(),
        movies: Vec::new(),
    };
    root.visit_owned_values(|visit| match visit.value() {
        Value::Reference(reference) if reference.type_name() == CANVAS_TYPE => {
            let target = reference.original_target();
            found.canvases.push(FoundCanvas {
                field: Location::new(visit.steps()).to_string(),
                guid: reference.guid(),
                ordinal: visit.ordinal(),
                target_record: target.record_index(),
                stored_guid: target.instance_location().is_none(),
            });
        }
        Value::String(text) if visit.field_name() == "moviePath" && !text.is_empty() => {
            found.movies.push(FoundMovie {
                path: (*text).to_owned(),
                ordinal: visit.ordinal(),
            });
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
        assert_eq!(
            found.movies.first().map(|movie| movie.path.as_str()),
            Some(MOVIE)
        );
        assert_eq!(
            found.canvases.first().map(|canvas| canvas.field.as_str()),
            Some(format!("{}canvas", "child.".repeat(DEPTH + 1)).as_str()),
        );
        assert_eq!(found.canvases.len(), 1);
        let assets = WidgetAssets::scan(tree.root());
        assert_eq!(
            assets.movies.first().map(|asset| asset.value.as_str()),
            Some(MOVIE)
        );
        Ok(())
    }
}
