// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;
use std::str::FromStr;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A separately selectable provider output stream.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum MediaKind {
    /// Public Archive and Entry metadata.
    Archive,
    /// Model geometry and definitions.
    Model,
    /// Material definitions.
    Material,
    /// Texture storage and images.
    Texture,
    /// Shader source or compiled data.
    Shader,
    /// Animation samples.
    Animation,
    /// Animation definitions and references.
    AnimationDefinition,
    /// Audio media and samples.
    Audio,
    /// Audio control definitions.
    AudioControl,
}

impl MediaKind {
    /// Returns the stable command-line and file-name spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Archive => "archive",
            Self::Model => "model",
            Self::Material => "material",
            Self::Texture => "texture",
            Self::Shader => "shader",
            Self::Animation => "animation",
            Self::AnimationDefinition => "animation-definition",
            Self::Audio => "audio",
            Self::AudioControl => "audio-control",
        }
    }
}

impl fmt::Display for MediaKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for MediaKind {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "archive" => Ok(Self::Archive),
            "model" => Ok(Self::Model),
            "material" => Ok(Self::Material),
            "texture" => Ok(Self::Texture),
            "shader" => Ok(Self::Shader),
            "animation" => Ok(Self::Animation),
            "animation-definition" => Ok(Self::AnimationDefinition),
            "audio" => Ok(Self::Audio),
            "audio-control" => Ok(Self::AudioControl),
            _ => Err(format!("unknown media kind {value:?}")),
        }
    }
}
