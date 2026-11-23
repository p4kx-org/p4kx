// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::query::sink::{Reached, Sink};
use crate::query::source::{QueryByteUse, QuerySource, QueryUnavailable, TraversalInput};
use crate::query::walk::Place;
use crate::{DataForge, Value, ValueError};

pub(super) fn readable<'source>(
    forge: &'source DataForge<'source>,
    bytes: &[u8],
    input: TraversalInput,
    reached: &Reached<'source>,
    sink: &mut impl Sink<'source>,
) {
    if matches!(reached, Reached::Value(Value::Error(_))) {
        sink.route_bytes(forge, bytes, QueryByteUse::AttemptedTraversal(input));
    }
}

pub(super) fn missing<'source>(
    forge: &'source DataForge<'source>,
    place: Place<'source>,
    at: u64,
    len: u64,
    input: TraversalInput,
    error: ValueError,
    sink: &mut impl Sink<'source>,
) -> Reached<'source> {
    if sink.source_depth().is_some() {
        sink.route(unavailable(forge, place, at, len, input));
    }
    Reached::Value(Value::Error(error))
}

pub(super) fn unavailable(
    forge: &DataForge<'_>,
    place: Place<'_>,
    offset: u64,
    byte_count: u64,
    input: TraversalInput,
) -> QuerySource {
    QuerySource::Unavailable(QueryUnavailable::Field {
        owner_start: forge.byte_span(place.bytes()).map(|span| span.start),
        offset,
        byte_count,
        input,
    })
}
