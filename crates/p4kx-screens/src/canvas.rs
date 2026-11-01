// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::rc::Rc;

use p4kx_dataforge::{DataForge, Guid, RecordTree, StructValue};

use crate::bindings::{TextBinding, text_bindings};
use crate::error::ScreenError;
use crate::geometry::Dim;
use crate::reader::{as_struct, child, float, items, reference, text};
use crate::rest::Operations;
use crate::statics::static_arrays;
use crate::style::Entry;
use crate::style::parse_entries;
use crate::widget::{Parent, Target, Widget};

pub(crate) const CANVAS_TYPE: &str = "BuildingBlocks_Canvas";

#[derive(Debug, Clone)]
pub(crate) struct StyleRef {
    pub(crate) guid: Guid,
    pub(crate) record_path: String,
}

#[derive(Debug)]
pub(crate) struct BrandStyle {
    pub(crate) style: StyleRef,
    pub(crate) entries: Rc<[Entry]>,
}

/// A canvas read from its Record: the widgets of its scene followed by those of its library.
#[derive(Debug)]
pub(crate) struct Canvas {
    pub(crate) guid: Guid,
    pub(crate) record_path: String,
    pub(crate) size: [f32; 2],
    pub(crate) world_root: Option<[f32; 2]>,
    pub(crate) widgets: Vec<Widget>,
    pub(crate) scene_len: usize,
    pub(crate) roots: Vec<usize>,
    pub(crate) children: Vec<Vec<usize>>,
    pub(crate) brands: Vec<BrandStyle>,
    pub(crate) shared: Vec<StyleRef>,
    pub(crate) defaults: Rc<[Entry]>,
    pub(crate) embedded: Rc<[Entry]>,
    pub(crate) unreachable: Vec<usize>,
    pub(crate) text_bindings: HashMap<usize, TextBinding>,
    pub(crate) operations: Operations,
    pub(crate) arrays: HashMap<String, usize>,
}

const WORLD_UNIT_LIMIT: f32 = 3.0;

impl Canvas {
    /// Returns the name of the canvas: the last segment of its Record path.
    pub(crate) fn name(&self) -> &str {
        self.record_path
            .rsplit_once('/')
            .map_or(self.record_path.as_str(), |(_, name)| name)
    }

    /// Returns the brand style whose Record path holds `s_<code>_`, ignoring case.
    pub(crate) fn brand_of(&self, code: &str) -> Option<&BrandStyle> {
        let wanted = format!("s_{}_", code.to_ascii_lowercase());
        self.brands.iter().find(|brand| {
            brand
                .style
                .record_path
                .to_ascii_lowercase()
                .contains(&wanted)
        })
    }

    pub(crate) fn pick_brand(&self, code: Option<&str>) -> Option<&BrandStyle> {
        code.and_then(|code| self.brand_of(code))
            .or_else(|| self.brands.first())
    }

    /// Returns the index in `widgets` of the widget a clone or list repeats.
    pub(crate) fn target_index(&self, target: Target) -> Option<usize> {
        let index = match target {
            Target::Scene(index) => index,
            Target::Library(index) => self.scene_len.checked_add(index)?,
        };
        (index < self.widgets.len()).then_some(index)
    }

    pub(crate) fn read(tree: &RecordTree<'_>) -> Self {
        let root = tree.root();
        let guid = tree.guid();
        let size = child(root, "size").map_or([0.0; 2], |size| {
            [
                float(size, "x").unwrap_or(0.0),
                float(size, "y").unwrap_or(0.0),
            ]
        });
        let raw = text(root, "coordinateMethod") == Some("useRaw");
        let parse = |value| as_struct(value).map(|widget| Widget::parse(widget, guid));
        let scene: Vec<Widget> = items(root, "scene").iter().filter_map(parse).collect();
        let scene_len = scene.len();
        let widgets: Vec<Widget> = scene
            .into_iter()
            .chain(items(root, "library").iter().filter_map(parse))
            .collect();
        let (roots, children) = hierarchy(&widgets, scene_len);
        let unreachable = unreachable_widgets(&widgets, scene_len, &roots, &children);
        Self {
            unreachable,
            text_bindings: text_bindings(root, scene_len),
            operations: Operations::read(root, scene_len),
            arrays: static_arrays(root),
            world_root: raw.then(|| world_root(&widgets, &roots)).flatten(),
            guid,
            record_path: tree.record_path().to_owned(),
            size,
            roots,
            children,
            scene_len,
            widgets,
            brands: brand_styles(root),
            shared: shared_styles(root),
            defaults: child(root, "defaultStyles")
                .map(|defaults| parse_entries(items(defaults, "entries")).into())
                .unwrap_or_default(),
            embedded: parse_entries(items(root, "embeddedStyles")).into(),
        }
    }
}

/// Returns the size of the largest root widget of fixed size when it is a few units only, which
/// makes the canvas a world-space canvas; roots of other sizes do not count.
fn world_root(widgets: &[Widget], roots: &[usize]) -> Option<[f32; 2]> {
    let [width, height] = roots
        .iter()
        .filter_map(|&index| match widgets.get(index)?.size {
            [Dim::Fixed(width), Dim::Fixed(height)] if width.is_finite() && height.is_finite() => {
                Some([width, height])
            }
            _ => None,
        })
        .max_by(|a, b| (a[0] * a[1]).total_cmp(&(b[0] * b[1])))?;
    let small = |value: f32| value > 0.0 && value <= WORLD_UNIT_LIMIT;
    (small(width) && small(height)).then_some([width, height])
}

/// Returns the widgets that neither a root of the scene nor a template without a parent in the
/// library reaches: their parents form a cycle or name no widget.
fn unreachable_widgets(
    widgets: &[Widget],
    scene_len: usize,
    roots: &[usize],
    children: &[Vec<usize>],
) -> Vec<usize> {
    let mut seen = vec![false; widgets.len()];
    let templates = (scene_len..widgets.len()).filter(|&index| {
        widgets
            .get(index)
            .is_some_and(|widget| !matches!(widget.parent, Parent::Library(_)))
    });
    let mut stack: Vec<usize> = roots.iter().copied().chain(templates).collect();
    while let Some(index) = stack.pop() {
        if let Some(flag) = seen.get_mut(index) {
            if *flag {
                continue;
            }
            *flag = true;
        }
        stack.extend(children.get(index).into_iter().flatten().copied());
    }
    (0..widgets.len())
        .filter(|index| !seen.get(*index).copied().unwrap_or(true))
        .collect()
}

fn hierarchy(widgets: &[Widget], scene_len: usize) -> (Vec<usize>, Vec<Vec<usize>>) {
    let mut roots = Vec::new();
    let mut children = vec![Vec::new(); widgets.len()];
    let mut attach = |parent: usize, index: usize| {
        if parent != index
            && let Some(list) = children.get_mut(parent)
        {
            list.push(index);
            true
        } else {
            false
        }
    };
    for (index, widget) in widgets.iter().enumerate() {
        let parent = match widget.parent {
            Parent::Widget(parent) if parent < scene_len => Some(parent),
            Parent::Library(parent) => scene_len.checked_add(parent),
            Parent::Root | Parent::Widget(_) => None,
        };
        let attached = parent.is_some_and(|parent| attach(parent, index));
        if !attached && index < scene_len && !matches!(widget.parent, Parent::Library(_)) {
            roots.push(index);
        }
    }
    let order = |index: &usize| widgets.get(*index).map_or(0, |widget| widget.order);
    roots.sort_by_key(order);
    for list in &mut children {
        list.sort_by_key(order);
    }
    (roots, children)
}

fn style_ref(structure: &StructValue<'_>, name: &str) -> Option<StyleRef> {
    reference(structure, name).map(|style| StyleRef {
        guid: style.guid(),
        record_path: style.record_path().to_owned(),
    })
}

fn brand_styles(root: &StructValue<'_>) -> Vec<BrandStyle> {
    items(root, "brandStyles")
        .iter()
        .filter_map(as_struct)
        .filter_map(|brand| {
            Some(BrandStyle {
                style: style_ref(brand, "brandIdentifier")?,
                entries: parse_entries(items(brand, "entries")).into(),
            })
        })
        .collect()
}

fn shared_styles(root: &StructValue<'_>) -> Vec<StyleRef> {
    let defaults =
        child(root, "defaultStyles").and_then(|defaults| style_ref(defaults, "sharedStyles"));
    defaults
        .into_iter()
        .chain(style_ref(root, "style"))
        .collect()
}

pub(crate) struct CanvasStore<'a, 'f> {
    forge: &'a DataForge<'f>,
    cache: HashMap<Guid, Option<Rc<Canvas>>>,
}

impl<'a, 'f> CanvasStore<'a, 'f> {
    pub(crate) fn new(forge: &'a DataForge<'f>) -> Self {
        Self {
            forge,
            cache: HashMap::new(),
        }
    }

    pub(crate) const fn forge(&self) -> &'a DataForge<'f> {
        self.forge
    }

    pub(crate) fn get(&mut self, guid: Guid) -> Option<Rc<Canvas>> {
        if let Some(cached) = self.cache.get(&guid) {
            return cached.clone();
        }
        let canvas = self
            .forge
            .record_by_guid(guid)
            .and_then(|record| record.tree().ok())
            .filter(|tree| tree.type_name() == CANVAS_TYPE)
            .map(|tree| Rc::new(Canvas::read(&tree)));
        self.cache.insert(guid, canvas.clone());
        canvas
    }
}

pub(crate) fn find_canvas(forge: &DataForge<'_>, text: &str) -> Result<Guid, ScreenError> {
    let unknown = || ScreenError::UnknownCanvas {
        text: text.to_owned(),
    };
    if let Ok(guid) = text.trim().parse::<Guid>() {
        let record = forge.record_by_guid(guid).ok_or_else(unknown)?;
        return if record.type_name().is_ok_and(|name| name == CANVAS_TYPE) {
            Ok(guid)
        } else {
            Err(ScreenError::NotACanvas {
                path: record.record_path().unwrap_or_default().to_owned(),
                type_name: record.type_name().unwrap_or_default().to_owned(),
            })
        };
    }
    let path = text.trim().replace('\\', "/");
    let path = path.strip_suffix(".xml").unwrap_or(&path);
    let matches: Vec<Guid> = forge
        .records_at_path(path)
        .filter(|record| record.type_name().is_ok_and(|name| name == CANVAS_TYPE))
        .map(|record| record.guid())
        .collect();
    match matches.as_slice() {
        [] => Err(unknown()),
        [guid] => Ok(*guid),
        many => Err(ScreenError::AmbiguousCanvas {
            text: text.to_owned(),
            count: many.len(),
        }),
    }
}
