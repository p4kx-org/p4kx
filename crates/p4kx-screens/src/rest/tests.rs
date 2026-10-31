// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::Rest;
use p4kx_dataforge::Guid;

use super::eval::Given;
use super::graph::{Binding, FieldKind, Node, Operations};
use super::ops::{Arith, Cmp};
use super::state::AtRest;
use crate::variables::Variable;

/// Evaluates `operations` under `inputs` and `variables`; no game variable names a canvas.
fn at_rest(operations: &Operations, inputs: &[(String, Rest)], variables: &[Variable]) -> AtRest {
    let given = Given {
        inputs,
        texts: &[],
        variables,
    };
    AtRest::resolving(operations, &given, &|_| {
        Err("a canvas that only the game names at run time".to_owned())
    })
}

fn binding(field: &str, kind: FieldKind, input: usize) -> Binding {
    Binding {
        widget: 0,
        field: field.to_owned(),
        kind,
        input: Some(input),
    }
}

fn graph(nodes: Vec<Node>, bindings: Vec<Binding>) -> Operations {
    Operations {
        nodes,
        bindings,
        kinds: vec!["Mystery".to_owned()],
    }
}

fn variable(binding: &str, zero: Rest) -> Node {
    Node::Variable {
        binding: binding.to_owned(),
        zero,
    }
}

fn flag() -> Node {
    variable("flag", Rest::Bool(false))
}

#[test]
fn a_cycle_is_unknown_and_does_not_loop() {
    let operations = graph(
        vec![Node::Invert(Some(1)), Node::Invert(Some(0))],
        vec![binding("IsActive", FieldKind::Boolean, 0)],
    );
    let state = at_rest(&operations, &[], &[]);
    assert_eq!(state.active(0), None);
    assert_eq!(state.unknown().len(), 1);
}

#[test]
fn a_chain_deeper_than_the_limit_is_unknown() {
    let mut nodes = vec![flag()];
    nodes.extend((0..400).map(|at| Node::Invert(Some(at))));
    let operations = graph(nodes, vec![binding("IsActive", FieldKind::Boolean, 400)]);
    let state = at_rest(&operations, &[], &[]);
    assert_eq!(state.active(0), None);
    assert!(
        state
            .unknown()
            .iter()
            .any(|(_, _, why)| why.contains("too deep"))
    );
}

#[test]
fn a_chain_within_the_limit_is_evaluated() {
    let mut nodes = vec![flag()];
    nodes.extend((0..100).map(|at| Node::Invert(Some(at))));
    let operations = graph(nodes, vec![binding("IsActive", FieldKind::Boolean, 100)]);
    assert_eq!(at_rest(&operations, &[], &[]).active(0), Some(false));
}

#[test]
fn an_input_that_does_not_exist_is_unknown() {
    let operations = graph(
        vec![Node::Invert(Some(7))],
        vec![binding("IsActive", FieldKind::Boolean, 0)],
    );
    assert_eq!(at_rest(&operations, &[], &[]).active(0), None);
}

#[test]
fn a_number_that_is_not_finite_is_unknown() {
    let operations = graph(
        vec![
            variable("level", Rest::Num(0.0)),
            Node::Arith {
                op: Arith::Div,
                amount: 0.0,
                left: Some(0),
                right: None,
                whole: false,
            },
            Node::Compare {
                cmp: Cmp::Equal,
                constant: 0.0,
                left: Some(1),
                right: None,
            },
        ],
        vec![binding("IsActive", FieldKind::Boolean, 2)],
    );
    assert_eq!(at_rest(&operations, &[], &[]).active(0), None);
}

#[test]
fn no_number_that_is_not_finite_reaches_an_alpha_or_a_parameter() {
    let operations = graph(
        vec![
            Node::Param {
                parameter: "ParamInput0".to_owned(),
                default: Rest::Num(f64::NAN),
            },
            Node::Same(Some(0)),
            Node::Choose {
                numbers: [0.0, f64::INFINITY],
                input: Some(3),
                branches: [None, None],
                whole: false,
            },
            flag(),
        ],
        vec![
            binding("Alpha", FieldKind::Number, 1),
            binding("ParamInput1", FieldKind::Number, 1),
            binding("Alpha", FieldKind::Number, 2),
        ],
    );
    let state = at_rest(&operations, &[], &[]);
    assert_eq!(state.alpha(0), None);
    assert_eq!(state.typed(0), []);
    assert_eq!(state.unknown().len(), 3);
    let clamped = graph(
        vec![
            variable("level", Rest::Num(0.5)),
            Node::Clamp {
                bounds: [f64::NEG_INFINITY, f64::INFINITY],
                input: Some(0),
                inputs: [None, None],
            },
        ],
        vec![binding("Alpha", FieldKind::Number, 1)],
    );
    assert_eq!(at_rest(&clamped, &[], &[]).alpha(0), Some(0.5));
}

#[test]
fn an_alpha_is_held_between_zero_and_one() {
    let operations = graph(
        vec![
            Node::Choose {
                numbers: [9.0, -3.0],
                input: Some(1),
                branches: [None, None],
                whole: false,
            },
            flag(),
        ],
        vec![binding("Alpha", FieldKind::Number, 0)],
    );
    assert_eq!(at_rest(&operations, &[], &[]).alpha(0), Some(0.0));
}

#[test]
fn the_deciding_input_of_and_and_or_wins_over_an_unknown_one() {
    let operations = graph(
        vec![
            flag(),
            Node::Unsupported(0),
            Node::All(vec![Some(0), Some(1)]),
            Node::Any(vec![Some(0), Some(1)]),
        ],
        vec![
            binding("IsActive", FieldKind::Boolean, 2),
            binding("Instantiated", FieldKind::Boolean, 3),
        ],
    );
    let state = at_rest(&operations, &[], &[]);
    assert_eq!(state.active(0), Some(false), "false and unknown");
    assert_eq!(state.instantiated(0), None, "false or unknown");
}

#[test]
fn a_parameter_of_another_kind_than_the_default_is_ignored() {
    let operations = graph(
        vec![Node::Param {
            parameter: "ParamInput0".to_owned(),
            default: Rest::Bool(true),
        }],
        vec![binding("IsActive", FieldKind::Boolean, 0)],
    );
    let wrong = [("ParamInput0".to_owned(), Rest::Num(0.0))];
    assert_eq!(at_rest(&operations, &wrong, &[]).active(0), Some(true));
    let right = [("ParamInput0".to_owned(), Rest::Bool(false))];
    assert_eq!(at_rest(&operations, &right, &[]).active(0), Some(false));
}

#[test]
fn a_variable_that_is_set_replaces_its_zero_and_the_last_setting_wins() {
    let operations = graph(
        vec![
            variable("PowerState", Rest::Num(0.0)),
            Node::Compare {
                cmp: Cmp::Equal,
                constant: 1.0,
                left: Some(0),
                right: None,
            },
        ],
        vec![binding("IsActive", FieldKind::Boolean, 1)],
    );
    let set = |value: &str| vec![Variable::new("powerstate", value).expect("variable")];
    assert_eq!(at_rest(&operations, &[], &set("1")).active(0), Some(true));
    assert_eq!(at_rest(&operations, &[], &set("2")).active(0), Some(false));
    let twice = [set("0"), set("1")].concat();
    assert_eq!(at_rest(&operations, &[], &twice).active(0), Some(true));
    let other = [Variable::new("other", "1").expect("variable")];
    assert_eq!(at_rest(&operations, &[], &other).active(0), Some(false));
}

#[test]
fn a_variable_that_does_not_fit_its_kind_is_unknown() {
    let operations = graph(
        vec![variable("PowerState", Rest::Num(0.0)), Node::Same(Some(0))],
        vec![binding("Alpha", FieldKind::Number, 1)],
    );
    let word = [Variable::new("powerstate", "on").expect("variable")];
    let state = at_rest(&operations, &[], &word);
    assert_eq!(state.alpha(0), None);
    assert!(
        state
            .unknown()
            .iter()
            .any(|(_, _, why)| why.contains("powerstate") && why.contains("not a number"))
    );
}

fn tag(byte: u8) -> Guid {
    Guid::from_stored([byte; 16])
}

fn state_tag(input: usize) -> Binding {
    binding("PrimaryStateTag", FieldKind::Text, input)
}

#[test]
fn a_boolean_picks_the_state_tag() {
    let choice = Node::TagChoice {
        tags: [Some(tag(1)), Some(tag(2))],
        branches: [None, None],
        input: Some(0),
    };
    let operations = graph(vec![flag(), choice], vec![state_tag(1)]);
    assert_eq!(
        at_rest(&operations, &[], &[]).state_tags(0),
        [tag(2)],
        "false takes the second tag"
    );
    let on = [Variable::new("flag", "true").expect("variable")];
    assert_eq!(at_rest(&operations, &[], &on).state_tags(0), [tag(1)]);
}

#[test]
fn a_branch_without_a_tag_gives_none() {
    let choice = Node::TagChoice {
        tags: [Some(tag(1)), None],
        branches: [None, None],
        input: Some(0),
    };
    let operations = graph(vec![flag(), choice], vec![state_tag(1)]);
    let state = at_rest(&operations, &[], &[]);
    assert_eq!(state.state_tags(0).first(), None);
    assert_eq!(state.unknown().first(), None);
}

#[test]
fn an_integer_switch_picks_its_pair_or_the_default() {
    let switch = Node::TagSwitch {
        pairs: vec![(1.0, Some(tag(3)))],
        default: Some(tag(4)),
        input: Some(0),
    };
    let operations = graph(
        vec![variable("level", Rest::Num(0.0)), switch],
        vec![state_tag(1)],
    );
    let set = |value: &str| [Variable::new("level", value).expect("variable")];
    assert_eq!(at_rest(&operations, &[], &set("1")).state_tags(0), [tag(3)]);
    assert_eq!(at_rest(&operations, &[], &set("2")).state_tags(0), [tag(4)]);
}

#[test]
fn a_tag_travels_through_a_string_parameter() {
    let operations = graph(
        vec![
            flag(),
            Node::Param {
                parameter: "ParamInput0".to_owned(),
                default: Rest::Text(String::new()),
            },
            Node::TagChoice {
                tags: [None, None],
                branches: [Some(1), Some(1)],
                input: Some(0),
            },
        ],
        vec![state_tag(2)],
    );
    let given = [("ParamInput0".to_owned(), Rest::Tag(Some(tag(5))))];
    assert_eq!(at_rest(&operations, &given, &[]).state_tags(0), [tag(5)]);
    assert_eq!(at_rest(&operations, &[], &[]).state_tags(0).first(), None);
}

#[test]
fn a_tag_the_game_names_at_run_time_is_unknown() {
    let operations = graph(
        vec![Node::Param {
            parameter: "ParamInput0".to_owned(),
            default: Rest::Text("UI.Style.Some".to_owned()),
        }],
        vec![state_tag(0)],
    );
    let state = at_rest(&operations, &[], &[]);
    assert_eq!(state.state_tags(0).first(), None);
    assert_eq!(state.unknown().len(), 1);
}

#[test]
fn a_tag_read_as_a_text_is_a_typed_unknown() {
    let operations = graph(
        vec![
            Node::Param {
                parameter: "ParamInput0".to_owned(),
                default: Rest::Text(String::new()),
            },
            Node::TextEmpty(Some(0)),
        ],
        vec![binding("IsActive", FieldKind::Boolean, 1)],
    );
    let given = [("ParamInput0".to_owned(), Rest::Tag(Some(tag(5))))];
    let state = at_rest(&operations, &given, &[]);
    assert_eq!(state.active(0), None);
    assert!(
        state
            .unknown()
            .iter()
            .any(|(_, _, why)| why.contains("not a text")),
        "{:?}",
        state.unknown()
    );
}

#[test]
fn parameters_tell_states_apart_by_key() {
    let none = at_rest(&graph(Vec::new(), Vec::new()), &[], &[]);
    let some = at_rest(
        &graph(Vec::new(), Vec::new()),
        &[("ParamInput0".to_owned(), Rest::Bool(true))],
        &[],
    );
    assert_eq!(none.key(), 0);
    assert_ne!(some.key(), 0);
}
