// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, HashSet};

use p4kx_dataforge::Guid;
use serde::Serialize;

use crate::assets::{AssetSet, ScreenAssets};
use crate::canvas::CanvasStore;
use crate::elements::is_rendered;
use crate::screen_asset_inputs::{ScreenAssetInputs, WidgetInput};
use crate::style::{CanvasStyles, StyleCache};

/// How many widgets of one element type a screen holds and whether P4KX draws them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ElementCount {
    /// The element type, e.g. `WidgetText`, without the `BuildingBlocks_` prefix.
    pub element: String,
    /// The widgets of this type, in every canvas of the screen.
    pub count: usize,
    /// Whether the renderer draws this type.
    pub rendered: bool,
}

struct Kind {
    count: usize,
    rendered: bool,
    widgets: Vec<WidgetInput>,
}

pub(crate) fn inspect(
    store: &mut CanvasStore<'_, '_>,
    root: Guid,
    mut inputs: Option<&mut ScreenAssetInputs>,
) -> (ScreenAssets, Vec<ElementCount>) {
    let mut assets = AssetSet::default();
    let mut kinds: BTreeMap<String, Kind> = BTreeMap::new();
    let mut seen = HashSet::new();
    let mut cache = StyleCache::new();
    let mut queue = vec![root];
    while let Some(guid) = queue.pop() {
        if !seen.insert(guid) {
            continue;
        }
        let Some(canvas) = store.get(guid) else {
            continue;
        };
        assets.add_canvas(&canvas.record_path, canvas.record_index);
        for widget in &canvas.widgets {
            assets.add(&widget.assets, widget.input);
            let name = widget
                .type_name
                .strip_prefix("BuildingBlocks_")
                .unwrap_or(&widget.type_name);
            let kind = kinds.entry(name.to_owned()).or_insert_with(|| Kind {
                count: 0,
                rendered: is_rendered(&widget.type_name),
                widgets: Vec::new(),
            });
            kind.count += 1;
            if inputs.is_some() {
                kind.widgets.push(widget.input);
            }
            queue.extend(widget.canvas);
        }
        queue.extend(CanvasStyles::swapped_canvases(
            store.forge(),
            &mut cache,
            &canvas,
        ));
    }
    let elements = kinds
        .into_iter()
        .map(|(element, kind)| {
            if let Some(inputs) = inputs.as_deref_mut() {
                inputs.elements.push(kind.widgets);
            }
            ElementCount {
                element,
                count: kind.count,
                rendered: kind.rendered,
            }
        })
        .collect();
    (assets.finish(inputs), elements)
}

#[cfg(test)]
mod tests {
    use p4kx_dataforge::DataForge;
    use p4kx_testkit::{
        DataForgeBuilder, TestCanvas, TestCondition, TestModifier, TestStyleEntry, TestValue,
        TestWidget, entity_schema, screens_schema,
    };

    use super::inspect;
    use crate::canvas::{CanvasStore, find_canvas};

    const OUTER: u128 = 0x5a00_0000_0000_4000_8000_0000_0000_0040;
    const SWAPPED: u128 = OUTER + 0x100;

    fn guid_text(guid: u128) -> String {
        format!(
            "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
            guid >> 96,
            (guid >> 80) & 0xffff,
            (guid >> 64) & 0xffff,
            (guid >> 48) & 0xffff,
            guid & 0xffff_ffff_ffff
        )
    }

    #[test]
    fn the_assets_of_a_canvas_a_style_swaps_in_are_listed() {
        let swap = TestStyleEntry::new("swap")
            .when(TestCondition::Type("Canvas"))
            .set(TestModifier::Canvas(SWAPPED));
        let outer = TestCanvas::new(OUTER, "outer", 10.0, 10.0)
            .embedded(swap)
            .widget(TestWidget::new("BuildingBlocks_WidgetCanvas", "holder"));
        let swapped = TestCanvas::new(SWAPPED, "swapped", 10.0, 10.0).widget(
            TestWidget::new("BuildingBlocks_WidgetImage", "picture")
                .with("imagePath", TestValue::text("UI/Test/swapped.tif")),
        );
        let bytes = [outer.record(), swapped.record()]
            .into_iter()
            .fold(
                screens_schema(entity_schema(DataForgeBuilder::new())),
                DataForgeBuilder::record,
            )
            .build();
        let forge = DataForge::open(&bytes).expect("database");
        let root = find_canvas(&forge, &guid_text(OUTER)).expect("canvas");
        let (assets, _) = inspect(&mut CanvasStore::new(&forge), root, None);
        assert_eq!(assets.canvases.len(), 2, "{:?}", assets.canvases);
        assert_eq!(assets.textures, ["UI/Test/swapped.dds"]);
    }

    #[test]
    fn more_than_4096_distinct_canvases_are_listed_and_cycles_terminate()
    -> Result<(), Box<dyn std::error::Error>> {
        const COUNT: usize = 4101;
        let source = (0..COUNT).fold(
            screens_schema(entity_schema(DataForgeBuilder::new())),
            |source, index| {
                let next = OUTER + ((index + 1) % COUNT) as u128;
                let mut canvas =
                    TestCanvas::new(OUTER + index as u128, format!("chain{index}"), 1.0, 1.0)
                        .widget(
                            TestWidget::new("BuildingBlocks_WidgetCanvas", "next")
                                .with("canvas", TestValue::Reference(next)),
                        );
                if index + 1 == COUNT {
                    canvas = canvas.widget(
                        TestWidget::new("BuildingBlocks_WidgetImage", "last")
                            .with("imagePath", TestValue::text("UI/Test/final.tif")),
                    );
                }
                source.record(canvas.record())
            },
        );
        let bytes = source.build();
        let forge = DataForge::open(&bytes)?;
        let root = find_canvas(&forge, &guid_text(OUTER))?;
        let (assets, kinds) = inspect(&mut CanvasStore::new(&forge), root, None);
        assert_eq!(assets.canvases.len(), COUNT);
        assert_eq!(assets.textures, ["UI/Test/final.dds"]);
        assert!(
            kinds
                .iter()
                .any(|kind| kind.element == "WidgetCanvas" && kind.count == COUNT)
        );
        Ok(())
    }
}
