// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::error::{self, Error, Kind};

const MAX_STEPS: usize = 64;
const MAX_VALUES: usize = 1_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub(crate) enum Step {
    Field { name: String },
    Each,
    Matching { field: String, value: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct Path(pub(crate) Vec<Step>);

impl Path {
    pub(crate) fn fields(names: &[&str]) -> Self {
        Self(
            names
                .iter()
                .map(|name| Step::Field {
                    name: (*name).to_owned(),
                })
                .collect(),
        )
    }

    pub(crate) fn validate(&self) -> super::Result<()> {
        if self.0.len() > MAX_STEPS {
            return Err(Error::new(Kind::Bounds, "projection path exceeds its step bound").into());
        }
        for step in &self.0 {
            let valid = |text: &str| {
                !text.is_empty() && text.len() <= 4096 && !text.chars().any(char::is_control)
            };
            let valid = match step {
                Step::Field { name } => valid(name),
                Step::Matching { field, value } => valid(field) && valid(value),
                Step::Each => true,
            };
            if !valid {
                return Err(Error::new(Kind::Bounds, "projection text exceeds its bound").into());
            }
        }
        Ok(())
    }

    pub(crate) fn select<'a>(
        &self,
        root: &'a Value,
        cancelled: &impl Fn() -> bool,
    ) -> super::Result<Vec<&'a Value>> {
        self.validate()?;
        let mut current = vec![root];
        for step in &self.0 {
            let mut next = Vec::new();
            for value in current {
                error::check(cancelled)?;
                match step {
                    Step::Field { name } => {
                        if let Some(value) = value.get(name) {
                            next.push(value);
                        }
                    }
                    Step::Each => {
                        if let Some(values) = value.as_array() {
                            if next.len().saturating_add(values.len()) > MAX_VALUES {
                                return Err(Error::new(
                                    Kind::Bounds,
                                    "projection result exceeds its bound",
                                )
                                .into());
                            }
                            next.extend(values);
                        }
                    }
                    Step::Matching {
                        field,
                        value: expected,
                    } => {
                        if let Some(values) = value.as_array() {
                            let matching = values.iter().filter(|value| {
                                value.get(field).and_then(Value::as_str) == Some(expected.as_str())
                            });
                            for value in matching {
                                if next.len() >= MAX_VALUES {
                                    return Err(Error::new(
                                        Kind::Bounds,
                                        "projection result exceeds its bound",
                                    )
                                    .into());
                                }
                                next.push(value);
                            }
                        }
                    }
                }
                if next.len() > MAX_VALUES {
                    return Err(
                        Error::new(Kind::Bounds, "projection result exceeds its bound").into(),
                    );
                }
            }
            current = next;
        }
        Ok(current)
    }
}
