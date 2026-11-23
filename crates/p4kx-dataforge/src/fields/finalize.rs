// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use rayon::prelude::*;

use super::part::Part;
use super::reduction::{FieldStatisticsReduction, ReducedNode};
use super::trie::{Node, ROOT};
use super::{FieldStat, FieldStatistics, order};

pub(super) fn ordinary(part: Part<'_, '_>) -> FieldStatistics {
    let (mut statistics, nodes) = into_nodes(part);
    statistics.fields = nodes_with_paths(nodes)
        .skip(1)
        .filter(|(node, _)| node.occurrences > 0)
        .map(|(node, path)| field(node, path))
        .collect();
    statistics
        .fields
        .par_sort_by(|a, b| order(&a.path, &b.path));
    statistics
}

pub(super) fn with_reduction<'source>(
    part: Part<'_, 'source>,
) -> FieldStatisticsReduction<'source> {
    let (mut statistics, nodes) = into_nodes(part);
    let mut captured = Vec::with_capacity(nodes.len());
    let mut rows: Vec<_> = nodes_with_paths(nodes)
        .enumerate()
        .filter_map(|(index, (mut node, path))| {
            let reduced = ReducedNode::take(&mut node, &captured);
            captured.push(reduced);
            if index == ROOT as usize || node.occurrences == 0 {
                None
            } else {
                Some((field(node, path), index))
            }
        })
        .collect();
    rows.par_sort_by(|(a, _), (b, _)| order(&a.path, &b.path));
    let (fields, rows) = rows.into_iter().unzip();
    statistics.fields = fields;
    FieldStatisticsReduction::new(statistics, rows, captured)
}

fn into_nodes<'source>(part: Part<'_, 'source>) -> (FieldStatistics, Vec<Node<'source>>) {
    let Part {
        trie,
        examined,
        unreadable_records,
        unreadable_values,
        ..
    } = part;
    (
        FieldStatistics {
            fields: Vec::new(),
            records: examined,
            unreadable_records,
            unreadable_values,
        },
        trie.nodes,
    )
}

fn nodes_with_paths(nodes: Vec<Node<'_>>) -> impl Iterator<Item = (Node<'_>, String)> {
    let paths = paths(&nodes);
    nodes.into_iter().zip(paths)
}

fn paths(nodes: &[Node<'_>]) -> Vec<String> {
    let mut paths: Vec<String> = Vec::with_capacity(nodes.len());
    for node in nodes {
        let parent = paths.get(node.parent as usize).map_or("", String::as_str);
        let mut path = String::with_capacity(parent.len() + 1 + node.segment.len());
        if node.parent != ROOT {
            path.push_str(parent);
            path.push('/');
        }
        path.push_str(node.segment);
        paths.push(path);
    }
    paths
}

fn field(mut node: Node<'_>, path: String) -> FieldStat {
    node.kinds.sort_by_key(|(kind, _)| kind.rank());
    FieldStat {
        path,
        occurrences: node.occurrences,
        records: node.records,
        empty: node.empty,
        kinds: node.kinds.into_boxed_slice(),
        distinct: node
            .distinct
            .as_ref()
            .map_or(0, |distinct| distinct.count()),
        distinct_capped: node
            .distinct
            .as_ref()
            .is_some_and(|distinct| distinct.is_capped()),
        min: node.extremes.as_ref().map(|extremes| extremes.min),
        max: node.extremes.as_ref().map(|extremes| extremes.max),
    }
}
