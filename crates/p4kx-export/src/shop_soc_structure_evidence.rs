// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use super::invalid_shape::InvalidShape;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(in super::super) enum Metric {
    UnparsedSoc,
    VerifiedSoc,
    ParsedTextEntityChunk,
    ParsedCryXmlEntityChunk,
    UnreadableEntityChunk,
    LossyEntityChunk,
    UninspectedChunk,
    DirectEntity,
    Component(Scope),
    ParameterNode,
    InventoryAttribute(InventoryValue),
    InventoryAttributeWithUniqueMetadataGuid(InventoryValue),
    MatchingInvalidShape(InvalidShape),
    AttributeWithoutReceipt,
    InventoryPropertyInOtherEntityScope,
    InventoryElementInEntity,
    Raw(RawKind, Relation),
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(in super::super) enum Scope {
    Properties,
    Components,
    Direct,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(in super::super) enum InventoryValue {
    Empty,
    File,
    Invalid,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(in super::super) enum RawKind {
    UnattachedPath,
    AttachedInvalid,
    UnattachedInvalid,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(in super::super) enum Relation {
    Total,
    StartsInLosslessEntityChunk,
    StartsInUninspectedChunk,
    AttributeName,
    AttributeValue,
    NodeName,
    NodeContent,
    ExactInventoryValue(InventoryValue),
    ExactInventoryValueWithUniqueMetadataGuid(InventoryValue),
    ExactInventoryValueWithSameAttachedGuid,
    ExactInventoryProperty(InventoryValue),
    ExactInventoryPropertyWithSameAttachedGuid(InventoryValue),
    ExactInventoryPropertyAndValue,
}

#[derive(Debug, Default)]
pub(in super::super) struct Evidence(BTreeMap<Metric, usize>);

impl Evidence {
    pub(in super::super) fn counts(&self) -> impl Iterator<Item = (Metric, usize)> + '_ {
        self.0.iter().map(|(&metric, &count)| (metric, count))
    }

    pub(super) fn add(&mut self, metric: Metric) {
        *self.0.entry(metric).or_default() += 1;
    }
}
