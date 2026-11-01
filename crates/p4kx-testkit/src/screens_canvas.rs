// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::screens_geometry::static_arrays;
use crate::screens_rest::{TestOp, operations};
use crate::screens_style::entries;
pub(crate) use crate::screens_style::{TestCondition, TestModifier, TestStyle, TestStyleEntry};
use crate::{TestInstance, TestRecord, TestValue, TestWidget};

const RECORD_FOLDER: &str = "libs/foundry/records/ui/buildingblocks/test/";

pub(crate) fn record_in(folder: &str, guid: u128, type_name: &str, name: &str) -> TestRecord {
    TestRecord::new(
        guid,
        type_name,
        format!("{type_name}.{name}"),
        format!("{folder}{name}.xml"),
    )
}

fn record(guid: u128, type_name: &str, name: &str) -> TestRecord {
    record_in(RECORD_FOLDER, guid, type_name, name)
}

/// Returns a Record of type `Tag` named `name`.
#[must_use]
pub fn tag_record(guid: u128, name: &str) -> TestRecord {
    record(guid, "Tag", name)
}

/// Returns a Record of type `Tag` whose tag path is `path`, as a root tag of its own.
#[must_use]
pub fn named_tag_record(guid: u128, path: &str) -> TestRecord {
    record(guid, "Tag", path).value("tagName", TestValue::text(path))
}

/// A synthetic `BuildingBlocks_Canvas` Record.
#[derive(Debug, Clone)]
pub struct TestCanvas {
    guid: u128,
    name: String,
    folder: &'static str,
    size: [f32; 2],
    method: &'static str,
    widgets: Vec<TestWidget>,
    templates: Vec<TestWidget>,
    arrays: Vec<(String, usize)>,
    operations: Vec<(String, TestOp)>,
    shared: Option<u128>,
    brand: Option<(u128, Vec<TestStyleEntry>)>,
    defaults: Vec<TestStyleEntry>,
    embedded: Vec<TestStyleEntry>,
}

impl TestCanvas {
    /// Creates a canvas of `width` x `height` pixels without widgets.
    #[must_use]
    pub fn new(guid: u128, name: impl Into<String>, width: f32, height: f32) -> Self {
        Self {
            guid,
            name: name.into(),
            folder: RECORD_FOLDER,
            size: [width, height],
            method: "auto",
            widgets: Vec::new(),
            templates: Vec::new(),
            arrays: Vec::new(),
            operations: Vec::new(),
            shared: None,
            brand: None,
            defaults: Vec::new(),
            embedded: Vec::new(),
        }
    }

    /// Puts the Record in the record folder `folder` (ending in `/`) instead of the test folder.
    #[must_use]
    pub const fn in_folder(mut self, folder: &'static str) -> Self {
        self.folder = folder;
        self
    }

    /// Makes the canvas use raw coordinates, whose fixed values count in pixels of its own size.
    #[must_use]
    pub const fn raw(self) -> Self {
        self.coordinates("useRaw")
    }

    /// Sets the coordinate method: `auto`, `useRaw`, `aspectOverridesWidth` or
    /// `aspectOverridesHeight`.
    #[must_use]
    pub const fn coordinates(mut self, method: &'static str) -> Self {
        self.method = method;
        self
    }

    /// Adds a widget to the library of the canvas, the templates that clones and lists
    /// instantiate; widgets whose parent it is are its children.
    #[must_use]
    pub fn template(mut self, widget: TestWidget) -> Self {
        self.templates.push(widget);
        self
    }

    /// Adds a static array variable of `rows` localised values.
    #[must_use]
    pub fn static_array(mut self, name: impl Into<String>, rows: usize) -> Self {
        self.arrays.push((name.into(), rows));
        self
    }

    /// Adds an entry to the `operations` of the canvas, the graph that binds game variables to
    /// fields of widgets; the operations it names as inputs must be added before it.
    #[must_use]
    pub fn operation(mut self, name: impl Into<String>, operation: TestOp) -> Self {
        self.operations.push((name.into(), operation));
        self
    }

    /// Adds a widget; its parent must be added before it.
    #[must_use]
    pub fn widget(mut self, widget: TestWidget) -> Self {
        self.widgets.push(widget);
        self
    }

    /// Names the style `guid` as the shared style of the canvas's default styles.
    #[must_use]
    pub const fn shared_style(mut self, guid: u128) -> Self {
        self.shared = Some(guid);
        self
    }

    /// Adds a brand style: the style Record `guid` and entries the canvas holds for it.
    #[must_use]
    pub fn brand_style(mut self, guid: u128, entries: Vec<TestStyleEntry>) -> Self {
        self.brand = Some((guid, entries));
        self
    }

    /// Adds an entry to the default styles of the canvas.
    #[must_use]
    pub fn default_style(mut self, entry: TestStyleEntry) -> Self {
        self.defaults.push(entry);
        self
    }

    /// Adds an embedded style entry.
    #[must_use]
    pub fn embedded(mut self, entry: TestStyleEntry) -> Self {
        self.embedded.push(entry);
        self
    }

    /// Returns the Record, at the record path `.../test/<name>`.
    #[must_use]
    pub fn record(&self) -> TestRecord {
        let size = TestValue::Struct(
            TestInstance::new("Vec3")
                .value("x", TestValue::Single(self.size[0]))
                .value("y", TestValue::Single(self.size[1]))
                .value("z", TestValue::Single(0.0)),
        );
        let defaults = TestInstance::new("BuildingBlocks_DefaultStyles")
            .value(
                "sharedStyles",
                self.shared.map_or(TestValue::Null, TestValue::Reference),
            )
            .value("entries", entries(&self.defaults));
        let brands = self.brand.iter().map(|(guid, brand_entries)| {
            TestValue::Struct(
                TestInstance::new("BuildingBlocks_BrandStyles")
                    .value("brandIdentifier", TestValue::Reference(*guid))
                    .value("entries", entries(brand_entries)),
            )
        });
        record_in(self.folder, self.guid, "BuildingBlocks_Canvas", &self.name)
            .value("coordinateMethod", TestValue::text(self.method))
            .value("size", size)
            .value("staticVariables", static_arrays(&self.arrays))
            .value("embeddedStyles", entries(&self.embedded))
            .value("defaultStyles", TestValue::Struct(defaults))
            .value("brandStyles", TestValue::Array(brands.collect()))
            .value(
                "library",
                TestValue::Array(self.templates.iter().map(TestWidget::value).collect()),
            )
            .value(
                "scene",
                TestValue::Array(self.widgets.iter().map(TestWidget::value).collect()),
            )
            .value("operations", operations(&self.operations))
    }
}

/// Returns an entity component that names the canvas `canvas` as the screen of its default view.
#[must_use]
pub fn screen_component(canvas: u128) -> TestValue {
    let view = TestInstance::new("BuildingBlocksView")
        .value("name", TestValue::text("_primary"))
        .value(
            "component",
            TestValue::Struct(
                TestInstance::new("UIGraph_BuildingBlocksContextComponent")
                    .value("canvas", TestValue::Reference(canvas)),
            ),
        );
    let layer = TestInstance::new("BuildingBlocksLayer")
        .value("layerName", TestValue::text("_primary"))
        .value("views", TestValue::Array(vec![TestValue::Struct(view)]));
    TestValue::Struct(
        TestInstance::new("UIBuildingBlocksEntityComponentParams")
            .value("layers", TestValue::Array(vec![TestValue::Struct(layer)])),
    )
}

/// Returns an entity component that names the UI movie `path`.
#[must_use]
pub fn movie_component(path: &str) -> TestValue {
    TestValue::Struct(
        TestInstance::new("UIOwnerEntityComponentParams").value("moviePath", TestValue::text(path)),
    )
}
