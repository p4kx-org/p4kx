// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::shader_declaration::{ShaderDeclaration, ShaderProperty};
use crate::shader_span::ShaderSpan;

/// Supported original source encodings, without rewritten payload bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum ShaderEncoding {
    /// UTF-8 without a leading byte-order marker.
    Utf8,
    /// UTF-8 with its original leading byte-order marker.
    Utf8Bom,
}

/// Source of the display shader name, without inventing an authored identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum ShaderNameOrigin {
    /// Original display filename stem exposed by the Archive reader.
    FileStem,
}

/// Complete observed units of one successfully structured extension source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct ShaderDefinitionCounts {
    /// This one definition source.
    #[schemars(range(min = 1, max = 1))]
    pub definitions: u64,
    /// Top-level Property blocks, independent of missing or duplicate Name fields.
    pub properties: u64,
    /// Every emitted declaration, including Property blocks and unknown nested scopes.
    pub declarations: u64,
    /// Complete top-level declaration count, including known and unknown names.
    pub root_declarations: u64,
    /// No compiled cache is decoded as a source definition.
    #[schemars(range(min = 0, max = 0))]
    pub compiled_caches: u64,
}

/// One source-definition header; all individual declarations use separate rows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct ShaderDefinition {
    /// Reader-exposed filename with its original display spelling.
    pub file_name: String,
    /// Display stem; physical source identity remains the complete container/Entry chain.
    pub shader_name: String,
    /// Explicit filename-derived origin of the display name.
    pub name_origin: ShaderNameOrigin,
    /// Original supported source encoding.
    pub encoding: ShaderEncoding,
    /// Entire retained Entry payload, including comments, whitespace and any BOM.
    pub span: ShaderSpan,
    /// Distinct complete counting units of the structured source.
    pub counts: ShaderDefinitionCounts,
}

/// Compiled family identified only by its selected source extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum ShaderCacheKind {
    /// Compiled include cache.
    Cfib,
    /// Compiled effect cache.
    Cfxb,
    /// Compiled shader cache.
    Shcb,
}

/// Honest semantic coverage of a retained compiled cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum ShaderCacheDecode {
    /// Inventory and raw bytes exist; the compiled payload has no semantic decoder.
    Unsupported,
}

/// Inventory of one retained compiled cache, without definition or Property claims.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct ShaderCache {
    /// Extension family, without a guessed binary layout version.
    pub family: ShaderCacheKind,
    /// Explicit unsupported semantic decoder.
    pub semantic_decode: ShaderCacheDecode,
    /// This one inventoried cache source.
    #[schemars(range(min = 1, max = 1))]
    pub compiled_caches: u64,
    /// No source definition was decoded from this payload.
    #[schemars(range(min = 0, max = 0))]
    pub definitions: u64,
    /// No Property was decoded from this payload.
    #[schemars(range(min = 0, max = 0))]
    pub properties: u64,
}

/// Structured source or budget failure without replacing retained bytes with defaults.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "reason", rename_all = "kebab-case", deny_unknown_fields)]
#[non_exhaustive]
pub enum ShaderFailureReason {
    /// Bytes cannot be interpreted as supported UTF-8.
    UnknownEncoding,
    /// A lexical or structural closing delimiter is absent.
    Unterminated {
        /// Exact bounded reader construct category.
        construct: ShaderConstruct,
    },
    /// A closing delimiter has no matching open construct.
    UnexpectedClosing {
        /// Exact bounded reader construct category.
        construct: ShaderConstruct,
    },
    /// Bounded source reading or syntax storage refused the next unit.
    Limit {
        /// Precise exhausted domain budget.
        budget: ShaderBudget,
        /// Actual configured bound.
        limit: u64,
    },
    /// The bounded reader could not allocate its next storage unit.
    Allocation,
    /// A newer reader diagnosis has no supported structured mapping.
    UnsupportedReaderVariant {
        /// Preserved diagnostic category spelling.
        detail: String,
    },
}

/// Lexical and structural construct categories of a definition failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum ShaderConstruct {
    /// Block comment.
    Comment,
    /// Quoted token.
    Quote,
    /// Braced scope.
    Brace,
    /// Parenthesised value.
    Parenthesis,
}

/// Bounded domain resources; independent of central media generation budgets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum ShaderBudget {
    /// Input source bytes.
    SourceBytes,
    /// One atom, quoted token or comment.
    TokenBytes,
    /// One declaration value.
    ValueBytes,
    /// Lexical token count.
    Tokens,
    /// Ordered declaration count.
    Declarations,
    /// Braced Property count across scopes.
    Properties,
    /// Combined braces and parentheses.
    Depth,
}

/// Failed structured projection whose exact raw Entry is retained independently.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct ShaderDefinitionFailure {
    /// Reader-exposed filename spelling.
    pub file_name: String,
    /// Exact structural or budget diagnosis.
    pub failure: ShaderFailureReason,
    /// Original Entry byte range at which projection failed.
    pub span: ShaderSpan,
    /// Definition/Property/declaration counts are unavailable, rather than fabricated as zero.
    pub counts: (),
}

/// Typed payloads produced by the built-in shader extension provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "record_type",
    content = "payload",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
#[non_exhaustive]
pub enum ShaderPayload {
    /// Successfully structured definition source header.
    Definition(ShaderDefinition),
    /// One source declaration outside the top-level Property row namespace.
    Declaration(ShaderDeclaration),
    /// One ordered top-level Property block.
    Property(ShaderProperty),
    /// One retained cache with explicitly unsupported semantics.
    CompiledCache(ShaderCache),
    /// A selected extension source could not be structurally projected.
    DefinitionFailure(ShaderDefinitionFailure),
}
