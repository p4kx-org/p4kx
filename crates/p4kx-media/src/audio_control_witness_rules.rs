// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::sound;
use serde_json::{Value, json};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Family {
    Trigger,
    Rtpc,
    Switch,
    Environment,
    Preload,
    External,
}

impl Family {
    pub(super) const ALL: [Self; 6] = [
        Self::Trigger,
        Self::Rtpc,
        Self::Switch,
        Self::Environment,
        Self::Preload,
        Self::External,
    ];

    pub(super) const fn name(self) -> &'static str {
        match self {
            Self::Trigger => "trigger",
            Self::Rtpc => "rtpc",
            Self::Switch => "switch",
            Self::Environment => "environment",
            Self::Preload => "preload",
            Self::External => "external",
        }
    }
}

#[derive(Clone, Copy)]
enum ValueKind {
    Text,
    Number,
    Boolean,
    Event,
}

pub(super) struct Rule {
    pub(super) family: Family,
    pub(super) tag: &'static str,
    pub(super) attribute: &'static str,
    pub(super) field: &'static str,
    pub(super) check: &'static str,
    kind: ValueKind,
}

impl Rule {
    const fn new(
        family: Family,
        tag: &'static str,
        attribute: &'static str,
        field: &'static str,
        check: &'static str,
    ) -> Self {
        Self {
            family,
            tag,
            attribute,
            field,
            check,
            kind: ValueKind::Text,
        }
    }

    const fn value_kind(mut self, kind: ValueKind) -> Self {
        self.kind = kind;
        self
    }

    pub(super) fn typed(&self, value: &str) -> Value {
        match self.kind {
            ValueKind::Text => json!({"type":"text","value":value}),
            Number => value
                .parse::<f64>()
                .ok()
                .filter(|number| number.is_finite())
                .map_or_else(
                    || json!({"type":"invalid-number"}),
                    |number| json!({"type":"number","value":number}),
                ),
            Boolean => match value.to_ascii_lowercase().as_str() {
                "true" => json!({"type":"boolean","value":true}),
                "false" => json!({"type":"boolean","value":false}),
                _ => json!({"type":"invalid-boolean"}),
            },
            Event => json!({
                "type":"wwise-event",
                "value":{"name":value,"id":sound::wwise_id(value)},
            }),
        }
    }
}

use Family::{Environment, External, Preload, Rtpc, Switch, Trigger};
use ValueKind::{Boolean, Event, Number};

pub(super) const RULES: &[Rule] = &[
    Rule::new(Trigger, "ATLTrigger", "path", "path", "trigger-path"),
    Rule::new(
        Trigger,
        "WwiseEvent",
        "wwise_name",
        "wwise-name",
        "event-name",
    )
    .value_kind(Event),
    Rule::new(
        Trigger,
        "WwiseEvent",
        "wwise_radius_max",
        "radius-max",
        "event-bound",
    )
    .value_kind(Number),
    Rule::new(
        Trigger,
        "WwiseEvent",
        "wwise_duration_min",
        "duration-min",
        "event-bound",
    )
    .value_kind(Number),
    Rule::new(
        Trigger,
        "WwiseEvent",
        "wwise_duration_max",
        "duration-max",
        "event-bound",
    )
    .value_kind(Number),
    Rule::new(
        Trigger,
        "WwiseEvent",
        "wwise_localised",
        "localised",
        "event-bound",
    )
    .value_kind(Boolean),
    Rule::new(
        Trigger,
        "WwiseEvent",
        "wwise_localized",
        "localised",
        "event-bound",
    )
    .value_kind(Boolean),
    Rule::new(
        Rtpc,
        "WwiseRtpc",
        "listener_rtpc",
        "listener-rtpc",
        "rtpc-listener",
    ),
    Rule::new(
        Rtpc,
        "WwiseRtpc",
        "rtpc_group_name",
        "rtpc-group-name",
        "rtpc-group",
    ),
    Rule::new(Rtpc, "WwiseRtpc", "min", "rtpc-min", "rtpc-min").value_kind(Number),
    Rule::new(Rtpc, "WwiseRtpc", "max", "rtpc-max", "rtpc-max").value_kind(Number),
    Rule::new(Rtpc, "WwiseRtpc", "default", "rtpc-default", "rtpc-default").value_kind(Number),
    Rule::new(Switch, "ATLSwitchState", "atl_name", "name", "switch-state"),
    Rule::new(
        Switch,
        "WwiseSwitch",
        "wwise_name",
        "wwise-name",
        "switch-binding",
    ),
    Rule::new(
        Switch,
        "WwiseState",
        "wwise_name",
        "wwise-name",
        "switch-binding",
    ),
    Rule::new(
        Switch,
        "WwiseValue",
        "wwise_name",
        "wwise-name",
        "switch-binding",
    ),
    Rule::new(
        Environment,
        "ATLEnvironment",
        "atl_name",
        "name",
        "environment-name",
    ),
    Rule::new(
        Environment,
        "WwiseAuxBus",
        "wwise_name",
        "wwise-name",
        "environment-target",
    ),
    Rule::new(
        Preload,
        "ATLPreloadRequest",
        "atl_name",
        "name",
        "preload-request",
    ),
    Rule::new(Preload, "ATLPreload", "atl_name", "name", "preload-link"),
    Rule::new(
        Preload,
        "Platform",
        "atl_config_group_name",
        "config-group-name",
        "platform-group",
    ),
    Rule::new(
        Preload,
        "ATLRTPCDependee",
        "atl_name",
        "name",
        "preload-dependee",
    ),
    Rule::new(
        External,
        "ATLExternalSource",
        "is_localised",
        "localised",
        "external-localisation",
    )
    .value_kind(Boolean),
    Rule::new(
        External,
        "ATLExternalSourceEntry",
        "language",
        "language",
        "external-language",
    ),
    Rule::new(
        External,
        "WwiseExternalSource",
        "wwise_filename",
        "external-filename",
        "external-file",
    ),
];
