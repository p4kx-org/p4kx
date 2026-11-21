// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{EntityMaterialEffects, FieldOrigin};
use serde_json::Value;

use super::{Binding, FieldOrigins, FieldPath, FieldPathError, GlowSelection};

/// The original array holding an authored Glow effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum GlowEffectKind {
    /// The material effect's tag effect array.
    Tag,
    /// The material effect's trigger effect array.
    Trigger,
}

impl GlowEffectKind {
    const fn field(self) -> &'static str {
        match self {
            Self::Tag => "tagEffects",
            Self::Trigger => "triggerEffects",
        }
    }
}

/// One original Glow property link and its independent geometry resolution.
#[derive(Debug)]
#[non_exhaustive]
pub struct EntityGlowLink {
    /// Original component slot.
    pub component: usize,
    /// Original tag or trigger effect array.
    pub kind: GlowEffectKind,
    /// Original slot within the effect array.
    pub effect: usize,
    /// Original slot within the property's link array.
    pub link: usize,
    /// Whether the original containing material group declares itself enabled.
    pub group_enabled: Option<bool>,
    /// Whether the original effect declares itself enabled.
    pub effect_enabled: Option<bool>,
    /// Exact selector string when authored as text.
    pub selector: Option<String>,
    /// Sourced selector pointer relative to its retained component declaration.
    pub selector_origin: Option<FieldOrigin>,
    /// Source geometry disposition, independent of dynamic effect conditions.
    pub selection: GlowSelection,
}

pub(super) fn read(
    source: &EntityMaterialEffects,
    binding: &Binding<'_>,
) -> Result<Vec<EntityGlowLink>, FieldPathError> {
    let mut links = Vec::new();
    for component in &source.components {
        for kind in [GlowEffectKind::Tag, GlowEffectKind::Trigger] {
            let effects = array(&component.declaration, kind.field());
            for (effect, value) in effects.iter().enumerate() {
                let rows = array(value, "propertyLinks");
                for (link, row) in rows.iter().enumerate() {
                    if row.get("materialProperty").and_then(Value::as_str) != Some("Glow") {
                        continue;
                    }
                    let raw_selector = row.get("subMeshName");
                    let selector = raw_selector.and_then(Value::as_str);
                    let selection = if raw_selector.is_some() && selector.is_none() {
                        GlowSelection::InvalidSelector
                    } else {
                        binding.select(selector)
                    };
                    links.push(EntityGlowLink {
                        component: component.component,
                        kind,
                        effect,
                        link,
                        group_enabled: enabled(&component.declaration),
                        effect_enabled: enabled(value),
                        selector: selector.map(str::to_owned),
                        selector_origin: origin(&component.field_origins, kind, effect, link)?,
                        selection,
                    });
                }
            }
        }
    }
    Ok(links)
}

fn array<'a>(row: &'a Value, name: &str) -> &'a [Value] {
    row.get(name)
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

fn enabled(row: &Value) -> Option<bool> {
    row.get("enabled").and_then(Value::as_bool)
}

fn origin(
    fields: &FieldOrigins,
    kind: GlowEffectKind,
    effect: usize,
    link: usize,
) -> Result<Option<FieldOrigin>, FieldPathError> {
    let pointer = FieldPath::parse(&format!(
        "/{}/{effect}/propertyLinks/{link}/subMeshName",
        kind.field()
    ))?;
    Ok(fields.get(&pointer).cloned())
}
