// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fmt;

#[derive(Clone, Copy)]
pub(super) enum Stage {
    Setup,
    Archive,
    Changed,
    Read,
    Limit,
    Material,
    Serialize,
    Compare,
    Coverage,
}

pub(super) struct Fault {
    stage: Stage,
    original: Option<Box<dyn Error>>,
}

impl Fault {
    pub(super) fn new(stage: Stage) -> Self {
        Self {
            stage,
            original: None,
        }
    }

    pub(super) fn with(stage: Stage, original: impl Error + 'static) -> Self {
        Self {
            stage,
            original: Some(Box::new(original)),
        }
    }

    pub(super) fn original(&self) -> Option<&(dyn Error + 'static)> {
        self.original.as_deref()
    }
}

impl fmt::Display for Fault {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let stage = match self.stage {
            Stage::Setup => "setup",
            Stage::Archive => "archive",
            Stage::Changed => "source-changed",
            Stage::Read => "source-read",
            Stage::Limit => "input-limit",
            Stage::Material => "material-read",
            Stage::Serialize => "serialization",
            Stage::Compare => "retained-field-compare",
            Stage::Coverage => "coverage",
        };
        write!(formatter, "material-source-census/{stage}")
    }
}

impl fmt::Debug for Fault {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

impl Error for Fault {}
