// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::shader_mask::ShaderMask;
use crate::shader_span::{ShaderSpan, ShaderText};

/// Authored declaration shape, independent of marker or value interpretation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum ShaderForm {
    /// Bare authored marker.
    Marker,
    /// Equals value, including explicit empty data.
    Equals,
    /// Parenthesised value, including explicit empty data.
    Parentheses,
    /// Braced body whose children have individual rows.
    Block,
    /// Source-preserved shape outside the supported declaration profile.
    Unknown,
}

/// Source-local parent namespace of an ordered declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum ShaderParentNamespace {
    /// Top-level Property declaration.
    ShaderProperty,
    /// Other known or unknown declaration.
    ShaderDeclaration,
}

/// Parent identity within the same physical Entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct ShaderParent {
    /// Namespace of the emitted parent row.
    pub namespace: ShaderParentNamespace,
    /// Original complete declaration ordinal path.
    pub indices: Vec<u64>,
}

/// Typed meaning of one occurrence, without last-wins selection or inferred defaults.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "role", rename_all = "kebab-case", deny_unknown_fields)]
#[non_exhaustive]
pub enum ShaderMeaning {
    /// Raw source syntax with no assigned semantic interpretation.
    Unknown,
    /// Top-level braced Property whose child occurrences have separate rows.
    Property,
    /// Authored root Version; its raw value is retained without a guessed version grammar.
    Version,
    /// Authored root common-global-flags declaration; no inheritance is inferred.
    UsesCommonGlobalFlags,
    /// Original Property Name.
    Name,
    /// Original Property Mask with an explicit interpretation result.
    Mask {
        /// Supported integer value or precise noninterpretation state.
        interpretation: ShaderMask,
    },
    /// Original editor Property label.
    Label,
    /// Original Property Description.
    Description,
    /// Original dependency-set expression, not a filesystem path.
    DependencySet,
    /// Original dependency-reset expression, not a filesystem path.
    DependencyReset,
    /// Authored Hidden occurrence; absence creates no synthetic boolean.
    Hidden,
}

/// One bounded source-spanned declaration; child bodies are never embedded again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(extend("required" = ["span", "name", "form", "value", "literal", "meaning", "child_count", "parent"]))]
#[non_exhaustive]
pub struct ShaderDeclaration {
    /// Complete original construct, addressed through the retained Entry bytes.
    pub span: ShaderSpan,
    /// Exact name token, absent for unnamed or unrecognised constructs.
    pub name: Option<ShaderText>,
    /// Original authored form.
    pub form: ShaderForm,
    /// Complete raw value range, absent for marker and block forms.
    pub value: Option<ShaderText>,
    /// Sole lexical atom or quoted token when one exists; quotes and escapes remain raw.
    pub literal: Option<ShaderText>,
    /// Meaning established only for supported root or direct Property fields.
    pub meaning: ShaderMeaning,
    /// Number of original immediate children, each emitted once as another row.
    pub child_count: u64,
    /// Parent row identity; absent for complete top-level declaration paths.
    pub parent: Option<ShaderParent>,
}

/// A top-level braced Property, independent of its optional name or mask fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct ShaderProperty {
    /// Zero-based order among top-level Property declarations, including duplicates.
    pub ordinal: u64,
    /// Original declaration and the count of separately emitted child fields.
    pub declaration: ShaderDeclaration,
}
