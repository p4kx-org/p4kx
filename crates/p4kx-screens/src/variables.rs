// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The game variables a caller sets before the bindings of a screen are evaluated (ADR-0259).

use std::str::FromStr;

use thiserror::Error;

use crate::ScreenError;
use crate::rest::Rest;

/// The most variables one composition takes.
pub const MAX_VARIABLES: usize = 64;

/// The longest name of a variable, in bytes.
pub const MAX_NAME_BYTES: usize = 128;

/// The longest value of a variable, in bytes.
pub const MAX_VALUE_BYTES: usize = 256;

/// Checks that `count` settings are no more than [`MAX_VARIABLES`], so that a caller can refuse
/// them before it reads anything.
///
/// # Errors
///
/// Returns [`ScreenError::TooManyVariables`] when there are more.
pub const fn check_variable_count(count: usize) -> Result<(), ScreenError> {
    if count > MAX_VARIABLES {
        return Err(ScreenError::TooManyVariables {
            count,
            max: MAX_VARIABLES,
        });
    }
    Ok(())
}

/// Why a variable setting was refused.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum VariableError {
    /// The setting has no `=` between name and value.
    #[error("a game variable is set as NAME=VALUE")]
    NoValue,
    /// The name has nothing but `/` or holds a control character.
    #[error("the name of a game variable is more than `/` and holds no control character")]
    BadName,
    /// The name is longer than [`MAX_NAME_BYTES`].
    #[error("the name of a game variable is at most {MAX_NAME_BYTES} bytes")]
    NameTooLong,
    /// The value holds a control character.
    #[error("the value of a game variable holds no control character")]
    BadValue,
    /// The value is longer than [`MAX_VALUE_BYTES`].
    #[error("the value of a game variable is at most {MAX_VALUE_BYTES} bytes")]
    ValueTooLong,
}

/// Separates the namespace of a setting, the canvas that reads the variable, from its name.
const NAMESPACE_SEPARATOR: &str = "::";

/// A game variable and the value it has when the bindings are evaluated, instead of its zero.
///
/// The bindings of a screen read game variables by path, such as `/seatdashboard/powerstate`; a
/// name without `/` is the last segment of that path, `powerstate`. A name may follow a
/// namespace and `::`, as in `mapdisplaymaster::isradar`: the variable is then set for the
/// bindings of that canvas only, named by the last segment of its Record path (ADR-0277). The
/// value is read as the kind of variable a binding names: `true`, `false` or a number for a
/// boolean, a finite number for a number or an integer, any text for a string.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Variable {
    name: String,
    value: String,
}

impl Variable {
    /// Creates a variable; the name and the value are trimmed.
    ///
    /// # Errors
    ///
    /// Returns the [`VariableError`] that says which bound the name or value breaks.
    pub fn new(name: &str, value: &str) -> Result<Self, VariableError> {
        let (name, value) = (name.trim(), value.trim());
        let (namespace, path) = split_namespace(name);
        let bad_namespace = namespace.is_some_and(|space| space.is_empty() || space.contains('/'));
        let bad_path = path.trim_matches('/').is_empty()
            || path.contains(NAMESPACE_SEPARATOR)
            || path.starts_with(':');
        if bad_namespace || bad_path || name.chars().any(char::is_control) {
            return Err(VariableError::BadName);
        }
        if name.len() > MAX_NAME_BYTES {
            return Err(VariableError::NameTooLong);
        }
        if value.chars().any(char::is_control) {
            return Err(VariableError::BadValue);
        }
        if value.len() > MAX_VALUE_BYTES {
            return Err(VariableError::ValueTooLong);
        }
        Ok(Self {
            name: name.to_owned(),
            value: value.to_owned(),
        })
    }

    /// Returns the name of the variable, as a binding of a screen names it.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the value as it was given.
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns whether a binding that reads the game variable at the path `binding`, such as
    /// `/seatdashboard/powerstate`, reads this variable; case is ignored.
    ///
    /// A name without `/` is the last segment of the path, one with `/` is the whole path.
    pub(crate) fn matches(&self, binding: &str) -> bool {
        let binding = binding.trim_matches('/');
        let name = split_namespace(&self.name).1.trim_matches('/');
        if name.contains('/') {
            binding.eq_ignore_ascii_case(name)
        } else {
            binding
                .rsplit('/')
                .next()
                .is_some_and(|last| last.eq_ignore_ascii_case(name))
        }
    }

    /// Returns whether the bindings of the canvas named `canvas`, the last segment of its Record
    /// path, can read this variable: it names no namespace, or this canvas; case is ignored.
    pub(crate) fn in_namespace(&self, canvas: &str) -> bool {
        split_namespace(&self.name)
            .0
            .is_none_or(|space| space.eq_ignore_ascii_case(canvas))
    }

    /// Reads the value as the kind of `zero`, the value the variable has at rest.
    pub(crate) fn coerce(&self, zero: &Rest) -> Result<Rest, String> {
        let refuse = |kind: &str| {
            format!(
                "the game variable {} is set to `{}`, which is not {kind}",
                self.name, self.value
            )
        };
        match zero {
            Rest::Bool(_) => match self.value.to_ascii_lowercase().as_str() {
                "true" => Ok(Rest::Bool(true)),
                "false" => Ok(Rest::Bool(false)),
                other => other
                    .parse::<f64>()
                    .ok()
                    .filter(|number| number.is_finite())
                    .map(|number| Rest::Bool(number != 0.0))
                    .ok_or_else(|| refuse("a boolean")),
            },
            Rest::Num(_) | Rest::Integer(_) => self
                .value
                .parse::<f64>()
                .ok()
                .filter(|number| number.is_finite())
                .map(Rest::Num)
                .ok_or_else(|| refuse("a number")),
            Rest::Text(_) => Ok(Rest::Text(self.value.clone())),
            Rest::Tag(_) => Err(refuse("a tag")),
        }
    }
}

/// Splits `name` into its namespace, if it names one, and the path of the variable.
fn split_namespace(name: &str) -> (Option<&str>, &str) {
    match name.split_once(NAMESPACE_SEPARATOR) {
        Some((space, path)) => (Some(space.trim()), path.trim()),
        None => (None, name),
    }
}

impl FromStr for Variable {
    type Err = VariableError;

    /// Reads `NAME=VALUE`.
    fn from_str(setting: &str) -> Result<Self, Self::Err> {
        let (name, value) = setting.split_once('=').ok_or(VariableError::NoValue)?;
        Self::new(name, value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_setting_splits_at_the_first_equals_sign_and_is_trimmed() {
        let variable: Variable = " powerstate = 1=2 ".parse().expect("a setting");
        assert_eq!((variable.name(), variable.value()), ("powerstate", "1=2"));
    }

    #[test]
    fn a_setting_that_breaks_a_bound_is_refused() {
        assert_eq!("power".parse::<Variable>(), Err(VariableError::NoValue));
        assert_eq!("=1".parse::<Variable>(), Err(VariableError::BadName));
        assert_eq!("/=1".parse::<Variable>(), Err(VariableError::BadName));
        assert_eq!(" // =1".parse::<Variable>(), Err(VariableError::BadName));
        assert_eq!("a\u{7}=1".parse::<Variable>(), Err(VariableError::BadName));
        assert_eq!(
            "a=\n".parse::<Variable>(),
            Ok(Variable::new("a", "").expect("empty"))
        );
        assert_eq!(
            "a=1\u{0}2".parse::<Variable>(),
            Err(VariableError::BadValue)
        );
        let long = "n".repeat(MAX_NAME_BYTES + 1);
        assert_eq!(
            format!("{long}=1").parse::<Variable>(),
            Err(VariableError::NameTooLong)
        );
        let long = "v".repeat(MAX_VALUE_BYTES + 1);
        assert_eq!(
            format!("a={long}").parse::<Variable>(),
            Err(VariableError::ValueTooLong)
        );
    }

    #[test]
    fn a_name_is_the_last_segment_of_a_path_or_the_whole_path() {
        let short = Variable::new("PowerState", "1").expect("variable");
        assert!(short.matches("/seatdashboard/powerstate"));
        assert!(short.matches("powerstate"));
        assert!(!short.matches("/seatdashboard/powerstate/other"));
        assert!(!short.matches("/seatdashboard/power"));
        let whole = Variable::new("/seatdashboard/powerstate", "1").expect("variable");
        assert!(whole.matches("/SeatDashboard/PowerState"));
        assert!(whole.matches("seatdashboard/powerstate"));
        assert!(!whole.matches("/other/powerstate"));
        assert!(!whole.matches("powerstate"));
    }

    #[test]
    fn a_namespace_limits_a_setting_to_the_canvas_it_names() {
        let scoped: Variable = "MapDisplayMaster :: isradar=1".parse().expect("setting");
        assert!(scoped.in_namespace("mapdisplaymaster"));
        assert!(!scoped.in_namespace("other"));
        assert!(scoped.matches("StarMapData/CommonData/IsRadar"));
        let plain = Variable::new("isradar", "1").expect("variable");
        assert!(plain.in_namespace("anything"));
        let whole = Variable::new("canvas::/a/b", "1").expect("variable");
        assert!(whole.matches("a/b") && !whole.matches("b"));
    }

    #[test]
    fn a_namespace_that_is_empty_or_a_path_is_refused() {
        for setting in [
            "::power=1",
            "a/b::power=1",
            "canvas::=1",
            "canvas::/=1",
            "a::b::c=1",
            "a:::b=1",
        ] {
            assert_eq!(
                setting.parse::<Variable>(),
                Err(VariableError::BadName),
                "{setting}"
            );
        }
    }

    #[test]
    fn a_value_is_read_as_the_kind_the_binding_names() {
        let power = Variable::new("powerstate", "1").expect("variable");
        assert_eq!(power.coerce(&Rest::Num(0.0)), Ok(Rest::Num(1.0)));
        assert_eq!(power.coerce(&Rest::Bool(false)), Ok(Rest::Bool(true)));
        assert_eq!(
            power.coerce(&Rest::Text(String::new())),
            Ok(Rest::Text("1".to_owned()))
        );
        let flag = Variable::new("on", "FALSE").expect("variable");
        assert_eq!(flag.coerce(&Rest::Bool(true)), Ok(Rest::Bool(false)));
        let word = Variable::new("on", "yes").expect("variable");
        assert!(word.coerce(&Rest::Bool(false)).is_err());
        assert!(word.coerce(&Rest::Num(0.0)).is_err());
        let huge = Variable::new("on", "inf").expect("variable");
        assert!(huge.coerce(&Rest::Num(0.0)).is_err());
        assert!(huge.coerce(&Rest::Bool(false)).is_err());
    }
}
