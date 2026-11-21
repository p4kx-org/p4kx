// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::io::{Read, Seek};

use p4kx_audio::BankContainerChildren;

use schemars::JsonSchema;
use serde::ser::{Error as _, SerializeSeq};
use serde::{Serialize, Serializer};

use crate::hirc_view::StateRange;
use crate::source_value::SourcedValue;

use super::ObjectOrigins;
use super::reader::NodeReader;

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(in crate::hirc_view) struct ActorChildren<Rows> {
    #[serde(skip_serializing_if = "Option::is_none")]
    declared_count: Option<SourcedValue<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    checked_range: Option<SourcedValue<StateRange>>,
    #[schemars(with = "Vec<ActorChild>")]
    children: Rows,
}

pub(super) type ActorChildrenSchema = ActorChildren<Vec<ActorChild>>;

pub(super) struct ActorChildrenSource<'view, 'source, 'input, R> {
    origins: &'view ObjectOrigins,
    node: &'view RefCell<NodeReader<'source, 'input, R>>,
}

impl<'view, 'source, 'input, R> ActorChildrenSource<'view, 'source, 'input, R> {
    pub(super) fn new(
        origins: &'view ObjectOrigins,
        node: &'view RefCell<NodeReader<'source, 'input, R>>,
        reader: &NodeReader<'_, '_, R>,
    ) -> Option<Self> {
        matches!(reader, NodeReader::Actor(_)).then_some(Self { origins, node })
    }
}

impl<R: Read + Seek> Serialize for ActorChildrenSource<'_, '_, '_, R> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut node = self.node.try_borrow_mut().map_err(S::Error::custom)?;
        let NodeReader::Actor(actor) = &mut *node else {
            return Err(S::Error::custom(
                "the selected original object is not an ActorMixer",
            ));
        };
        let fields = actor.fields();
        let declared_count = self
            .origins
            .optional(fields.child_count.as_ref(), "actorMixerChildCount")
            .map_err(S::Error::custom)?;
        let checked_range = fields
            .children
            .as_ref()
            .map(|range| self.origins.range(range, "actorMixerChildrenExtent"))
            .transpose()
            .map_err(S::Error::custom)?;
        ActorChildren {
            declared_count,
            checked_range,
            children: ChildRows {
                origins: self.origins,
                rows: RefCell::new(actor.children()),
            },
        }
        .serialize(serializer)
    }
}

struct ChildRows<'view, 'input, R> {
    origins: &'view ObjectOrigins,
    rows: RefCell<Option<BankContainerChildren<'input, R>>>,
}

impl<R: Read + Seek> Serialize for ChildRows<'_, '_, R> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut rows = self.rows.try_borrow_mut().map_err(S::Error::custom)?;
        let mut output = serializer.serialize_seq(None)?;
        if let Some(rows) = rows.as_mut() {
            for child in rows {
                let child = child.map_err(S::Error::custom)?;
                output.serialize_element(&ActorChild {
                    ordinal: self
                        .origins
                        .derived(
                            child.ordinal,
                            &child.child_id.range,
                            "actorMixerChildPosition",
                        )
                        .map_err(S::Error::custom)?,
                    id: self
                        .origins
                        .field(&child.child_id, "actorMixerChildId")
                        .map_err(S::Error::custom)?,
                })?;
            }
        }
        output.end()
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(in crate::hirc_view) struct ActorChild {
    ordinal: SourcedValue<u32>,
    id: SourcedValue<u32>,
}
