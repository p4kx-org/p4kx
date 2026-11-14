// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

mod arc;
mod basic;
mod css;
mod cubic;
mod emit;
mod number;
mod path;
mod reference;
mod style;
mod transform;

use std::collections::HashMap;

use p4kx_gfx::{Affine, ShapeLayer};
use roxmltree::{Document, Node, ParsingOptions};
use thiserror::Error;

use self::css::{Sheet, declarations};
use self::emit::emit;
use self::number::number_list;
use self::reference::reference;
pub(crate) use self::style::Slot;
use self::style::{Colour, Style, colour, slot_of};

/// The most bytes of an SVG file that are read.
pub const MAX_SVG_BYTES: usize = 2 << 20;

const MAX_NODES: u32 = 1 << 16;
const MAX_DEPTH: usize = 32;
const MAX_COMMANDS: usize = 1 << 18;
const MAX_WORK: usize = 1 << 24;
const MAX_CSS_BYTES: usize = 1 << 18;
const PRESENTATION: [&str; 11] = [
    "fill",
    "stroke",
    "stroke-width",
    "fill-opacity",
    "stroke-opacity",
    "opacity",
    "stroke-linecap",
    "stroke-linejoin",
    "stroke-miterlimit",
    "display",
    "visibility",
];
const MAX_IDS: usize = 1 << 12;
const DOCTYPE_WINDOW: usize = 4096;

#[derive(Debug, Error)]
pub(crate) enum SvgError {
    #[error("the file is {0} bytes, more than the {MAX_SVG_BYTES} that are read")]
    TooLarge(usize),
    #[error("the file is not UTF-8 text")]
    NotText,
    #[error("the document declares entities, which are not read")]
    Entities,
    #[error("the document is not well-formed XML: {0}")]
    Xml(#[from] roxmltree::Error),
    #[error("the root element is `{0}`, not `svg`")]
    NotSvg(String),
    #[error("the document has no usable viewBox or size")]
    NoSize,
    #[error("a path or point list is malformed")]
    Path,
    #[error("the paths have more than {MAX_COMMANDS} commands")]
    TooManyCommands,
    #[error("the document draws too many elements: more than {MAX_WORK} units of work")]
    TooManyElements,
}

/// A parsed SVG: layers in user units, with the box they fit in.
#[derive(Debug, Clone)]
pub(crate) struct Vector {
    pub(crate) view: [f32; 4],
    pub(crate) layers: Vec<ShapeLayer>,
    pub(crate) slots: Vec<Option<Slot>>,
    pub(crate) notes: Vec<&'static str>,
    pub(crate) commands: usize,
    pub(crate) work: usize,
}

struct Walk<'a, 'input> {
    sheet: Sheet,
    gradients: HashMap<&'a str, Node<'a, 'input>>,
    ids: HashMap<&'a str, Node<'a, 'input>>,
    layers: Vec<ShapeLayer>,
    slots: Vec<Option<Slot>>,
    budget: usize,
    work: usize,
    matches: HashMap<roxmltree::NodeId, Vec<usize>>,
    notes: Vec<&'static str>,
}

impl Walk<'_, '_> {
    fn charge(&mut self, units: usize) -> Result<(), SvgError> {
        self.work = self
            .work
            .checked_sub(units)
            .ok_or(SvgError::TooManyElements)?;
        Ok(())
    }

    fn note(&mut self, note: &'static str) {
        if !self.notes.contains(&note) {
            self.notes.push(note);
        }
    }
}

fn strip_doctype(text: &str) -> Result<std::borrow::Cow<'_, str>, SvgError> {
    let window_end = text
        .char_indices()
        .map(|(at, _)| at)
        .find(|at| *at >= DOCTYPE_WINDOW)
        .unwrap_or(text.len());
    let window = text.get(..window_end).unwrap_or_default();
    let Some(start) = window.find("<!DOCTYPE") else {
        return Ok(text.into());
    };
    let declaration = text.get(start..).unwrap_or_default();
    let end = declaration.find('>').ok_or(SvgError::Entities)?;
    if declaration.get(..end).unwrap_or_default().contains('[') {
        return Err(SvgError::Entities);
    }
    let mut stripped = String::with_capacity(text.len());
    stripped.push_str(text.get(..start).unwrap_or_default());
    stripped.push_str(declaration.get(end + 1..).unwrap_or_default());
    Ok(stripped.into())
}

/// Parses an SVG file into layers, reading the subset the game uses: paths and basic shapes,
/// groups, transforms, solid fills and strokes, and the presentation attributes, `style`
/// attributes and simple `<style>` rules that set them.
///
/// Nothing is fetched: external references are ignored, and a document with entity declarations
/// is refused.
pub(crate) fn parse(bytes: &[u8]) -> Result<Vector, SvgError> {
    if bytes.len() > MAX_SVG_BYTES {
        return Err(SvgError::TooLarge(bytes.len()));
    }
    let text = std::str::from_utf8(bytes).map_err(|_| SvgError::NotText)?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let text = strip_doctype(text)?;
    let options = ParsingOptions {
        allow_dtd: false,
        nodes_limit: MAX_NODES,
        ..ParsingOptions::default()
    };
    let document = Document::parse_with_options(&text, options)?;
    let root = document.root_element();
    if root.tag_name().name() != "svg" {
        return Err(SvgError::NotSvg(root.tag_name().name().to_owned()));
    }
    let view = view_box(root).ok_or(SvgError::NoSize)?;
    let mut walk = Walk {
        sheet: Sheet::default(),
        gradients: HashMap::new(),
        ids: HashMap::new(),
        layers: Vec::new(),
        slots: Vec::new(),
        budget: MAX_COMMANDS,
        work: MAX_WORK,
        matches: HashMap::new(),
        notes: Vec::new(),
    };
    index(&document, &mut walk);
    visit(&mut walk, root, &Style::default(), &Affine::IDENTITY, 0)?;
    Ok(Vector {
        view,
        layers: walk.layers,
        slots: walk.slots,
        notes: walk.notes,
        commands: MAX_COMMANDS - walk.budget,
        work: MAX_WORK - walk.work,
    })
}

fn index<'a, 'input>(document: &'a Document<'input>, walk: &mut Walk<'a, 'input>) {
    let mut css_bytes = 0;
    for node in document.descendants().filter(Node::is_element) {
        if let Some(id) = node.attribute("id")
            && walk.ids.len() < MAX_IDS
        {
            walk.ids.insert(id, node);
        }
        match node.tag_name().name() {
            "style" => {
                for text in node.children().filter_map(|child| child.text()) {
                    css_bytes += text.len();
                    if css_bytes > MAX_CSS_BYTES || walk.sheet.is_full() {
                        walk.note("style rules beyond the limit are left out");
                        break;
                    }
                    walk.sheet.add(text);
                }
            }
            "linearGradient" | "radialGradient" => {
                if let Some(id) = node.attribute("id")
                    && walk.gradients.len() < MAX_IDS
                {
                    walk.gradients.insert(id, node);
                }
            }
            _ => {}
        }
    }
}

fn view_box(root: Node<'_, '_>) -> Option<[f32; 4]> {
    let valid = |values: &[f32]| match values {
        [x, y, w, h] if *w > 0.0 && *h > 0.0 => Some([*x, *y, *w, *h]),
        _ => None,
    };
    if let Some(found) = root
        .attribute("viewBox")
        .and_then(|text| number_list(text, 4))
        .and_then(|values| valid(&values))
    {
        return Some(found);
    }
    let size = |name: &str| root.attribute(name).and_then(number::leading_number);
    valid(&[0.0, 0.0, size("width")?, size("height")?])
}

fn first_stop(walk: &Walk<'_, '_>, id: &str) -> Option<Colour> {
    let mut node = *walk.gradients.get(id)?;
    for _ in 0..4 {
        if let Some(stop) = node.children().find(|c| c.tag_name().name() == "stop") {
            let mut found = stop.attribute("stop-color").and_then(colour);
            let mut opacity = stop.attribute("stop-opacity").map(str::to_owned);
            for (name, value) in stop.attribute("style").map_or_default(declarations) {
                match name.as_str() {
                    "stop-color" => found = colour(&value),
                    "stop-opacity" => opacity = Some(value),
                    _ => {}
                }
            }
            let mut found = found.unwrap_or(Colour {
                rgb: [0, 0, 0],
                alpha: 1.0,
            });
            if let Some(opacity) = opacity.as_deref().and_then(number::leading_number) {
                found.alpha *= opacity.clamp(0.0, 1.0);
            }
            return Some(found);
        }
        let link = node
            .attribute(("http://www.w3.org/1999/xlink", "href"))
            .or_else(|| node.attribute("href"))?;
        node = *walk.gradients.get(link.trim_start_matches('#'))?;
    }
    None
}

fn element_style(
    walk: &mut Walk<'_, '_>,
    node: Node<'_, '_>,
    inherited: &Style,
) -> Result<Style, SvgError> {
    let mut style = inherited.clone();
    style.base_opacity = inherited.opacity;
    if let Some(slot) = node.attribute("id").and_then(slot_of) {
        style.slot = Some(slot);
    }
    let key = node.id();
    if !walk.matches.contains_key(&key) {
        let rules = walk.sheet.matching(
            node.tag_name().name(),
            node.attribute("class").unwrap_or_default(),
            node.attribute("id").unwrap_or_default(),
        );
        walk.matches.insert(key, rules);
    }
    let rules = walk.matches.get(&key).map_or(&[][..], Vec::as_slice);
    let attributes: Vec<(&str, &str)> = PRESENTATION
        .iter()
        .filter_map(|name| Some((*name, node.attribute(*name)?)))
        .collect();
    let inline = node.attribute("style").unwrap_or_default();
    let cost = attributes
        .iter()
        .map(|(name, value)| name.len() + value.len())
        .sum::<usize>()
        + inline.len()
        + rules
            .iter()
            .map(|rule| walk.sheet.declared(*rule).len() * 16)
            .sum::<usize>()
        + PRESENTATION.len();
    walk.work = walk
        .work
        .checked_sub(cost)
        .ok_or(SvgError::TooManyElements)?;
    let mut notes: Vec<&'static str> = Vec::new();
    let gradients = |id: &str| first_stop(walk, id);
    let mut apply = |name: &str, value: &str| {
        style.set(name, value, &gradients, &mut |note| notes.push(note));
    };
    for (name, value) in &attributes {
        apply(name, value);
    }
    for rule in rules {
        for (name, value) in walk.sheet.declared(*rule) {
            apply(name, value);
        }
    }
    for (name, value) in declarations(inline) {
        apply(&name, &value);
    }
    for note in notes {
        walk.note(note);
    }
    Ok(style)
}

fn visit(
    walk: &mut Walk<'_, '_>,
    node: Node<'_, '_>,
    inherited: &Style,
    parent: &Affine,
    depth: usize,
) -> Result<(), SvgError> {
    if depth > MAX_DEPTH {
        walk.note("elements nested deeper than 32 levels are left out");
        return Ok(());
    }
    let style = element_style(walk, node, inherited)?;
    if style.hidden {
        return Ok(());
    }
    let local = node
        .attribute("transform")
        .map_or(Affine::IDENTITY, |text| {
            transform::parse(text).unwrap_or_else(|| {
                walk.note("a malformed transform is ignored");
                Affine::IDENTITY
            })
        });
    let total = parent.then(&local);
    for attribute in ["clip-path", "mask", "filter"] {
        if node.attribute(attribute).is_some_and(|v| v != "none") {
            walk.note("clip paths, masks and filters are not applied");
        }
    }
    match node.tag_name().name() {
        "svg" | "g" | "a" | "switch" => {
            for child in node.children().filter(Node::is_element) {
                visit(walk, child, &style, &total, depth + 1)?;
            }
        }
        "defs" | "style" | "title" | "desc" | "metadata" | "linearGradient" | "radialGradient"
        | "clipPath" | "mask" | "symbol" | "marker" | "pattern" | "filter" => {}
        "use" => reference(walk, node, &style, &total, depth)?,
        "image" | "text" | "foreignObject" => {
            walk.note("image and text elements are left out");
        }
        _ => {
            let heavy = ["d", "points"]
                .iter()
                .filter_map(|name| node.attribute(*name))
                .map(str::len)
                .sum();
            walk.charge(heavy)?;
            if let Some(commands) = basic::commands(node, &mut walk.budget)? {
                emit(walk, &commands, &style, &total)?;
            }
        }
    }
    Ok(())
}
