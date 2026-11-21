// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::cryxml::{SourceDocument, SourceElement, SourceReadOptions};
use p4kx_extraction::{Archive, Entry};
use serde_json::{Value, json};

use crate::real_archive_item_durability::{number, rows, text};

use super::candidates;
use super::options::{self, Coverage, OptionRow};
use super::proof::{self, Inventory, XmlProof};
use super::scopes::{self, Scope};
use super::{Attachment, Motion, TestResult, unique};

struct Catalog<'catalog, 'read> {
    archive: &'catalog Archive,
    raw: &'catalog Value,
    candidates: &'catalog [Value],
    weapon_uses: &'catalog [Value],
    inventory: &'read Inventory<'catalog>,
}

pub(super) fn compare<'catalog>(
    archive: &'catalog Archive,
    raw: &'catalog Value,
    candidates: &'catalog Value,
    inventory: &Inventory<'catalog>,
    attachment: &Attachment<'_>,
    motion: &Motion<'_>,
    weapon_uses: &'catalog [Value],
) -> TestResult<Coverage> {
    let catalog = Catalog {
        archive,
        raw,
        inventory,
        candidates: rows(candidates, "/rows")?,
        weapon_uses,
    };
    let parent = proof::resolve(archive, motion.database)?;
    let controller = proof::resolve(archive, motion.controller)?;
    catalog.actor_bindings(motion, parent, controller)?;
    catalog.controller(controller)?;
    let bytes = proof::read_bytes(archive, parent)?;
    let document = parse(&bytes)?;
    let root = document.root();
    let row = document_row(raw, "/databases", parent)?;
    let proof = inventory.xml(row, parent)?;
    proof.position(&row["position"], root)?;
    proof.attributes(row, root)?;
    let fragments = scopes::definition(archive, scopes::source_attribute(root, "FragDef"))?;
    catalog.scopes(parent, &fragments, None)?;
    let context = BranchOwner {
        actual: parent,
        exported: row,
        proof,
        fragments,
    };
    let mut counts = Coverage::default();
    for (section, element) in root
        .elements()
        .enumerate()
        .filter(|(_, node)| named(*node, "SubADBs"))
    {
        for (ordinal, branch) in element
            .elements()
            .enumerate()
            .filter(|(_, node)| named(*node, "SubADB"))
        {
            let names = selectors(branch);
            if !names.is_empty() {
                catalog.branch(
                    &context,
                    branch,
                    [section, ordinal],
                    &names,
                    attachment,
                    motion,
                    &mut counts,
                )?;
            }
        }
    }
    assert!(
        counts.branches > 0 && counts.options > 0 && counts.blends > 0 && counts.animations > 0,
        "the initialized witness reaches actual original reload options and timing fields",
    );
    Ok(counts)
}

struct BranchOwner<'catalog, 'read> {
    actual: &'read Entry,
    exported: &'catalog Value,
    proof: XmlProof<'catalog, 'read>,
    fragments: Scope,
}

impl<'catalog> Catalog<'catalog, '_> {
    fn actor_bindings(&self, motion: &Motion<'_>, adb: &Entry, controller: &Entry) -> TestResult {
        let bind = unique(
            rows(self.raw, "/actorBindings")?.iter().filter(|row| {
                row.pointer("/record/value").and_then(Value::as_u64)
                    == Some(u64::from(motion.index))
                    && row.pointer("/component/value").and_then(Value::as_u64)
                        == u64::try_from(motion.ordinal).ok()
            }),
            "exact physical Player binding",
        )?;
        self.link(&bind["database"], adb)?;
        self.link(&bind["controller"], controller)
    }

    fn link(&self, link: &Value, entry: &Entry) -> TestResult {
        assert!(
            number(link, "/source/entryIndex")? == u64::try_from(entry.index())?
                && text(link, "/state/value")? == "read",
            "selector resolves to the exact verified Archive Entry",
        );
        self.inventory
            .verify_entry(text(link, "/source/inputRef")?, entry)?;
        self.inventory.closure(&link["state"])
    }

    fn controller(&self, entry: &Entry) -> TestResult {
        let bytes = proof::read_bytes(self.archive, entry)?;
        let document = parse(&bytes)?;
        let root = document.root();
        assert!(
            named(root, "ControllerDef"),
            "original Player action controller"
        );
        let catalog = document_row(self.raw, "/controllers", entry)?;
        let proof = self.inventory.xml(catalog, entry)?;
        for (child, path) in [("Tags", "/tags"), ("Fragments", "/fragments")] {
            let projected = rows(catalog, path)?;
            let actual = scopes::children(root, child).collect::<Vec<_>>();
            assert!(
                projected.len() == actual.len(),
                "original controller selectors"
            );
            for (entry, node) in projected.iter().zip(actual) {
                proof.position(&entry["position"], node)?;
                proof.attributes(entry, node)?;
            }
        }
        Ok(())
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "the witness compares one original branch and two chosen owner coordinates"
    )]
    fn branch(
        &self,
        owner: &BranchOwner<'catalog, '_>,
        branch: SourceElement<'_>,
        ordinals: [usize; 2],
        names: &[&str],
        attachment: &Attachment<'_>,
        motion: &Motion<'_>,
        counts: &mut Coverage,
    ) -> TestResult {
        counts.branches += 1;
        let position = proof::node_location(branch, "$node")?;
        original_branch(owner, branch, &position, ordinals)?;
        let link = unique(
            rows(self.raw, "/branches")?.iter().filter(|row| {
                row.pointer("/position/value") == Some(&position)
                    && row.pointer("/source/entryIndex").and_then(Value::as_u64)
                        == u64::try_from(owner.actual.index()).ok()
            }),
            "physical branch link",
        )?;
        owner.proof.position(&link["position"], branch)?;
        assert!(
            text(link, "/traversal/value")? == "resolved",
            "verified direct reload branch"
        );
        let child = proof::resolve(
            self.archive,
            scopes::source_attribute(branch, "File").ok_or("original SubADB File selector")?,
        )?;
        self.link(&link["target"], child)?;
        let bytes = proof::read_bytes(self.archive, child)?;
        let document = parse(&bytes)?;
        let root = document.root();
        assert!(named(root, "AnimDB"), "original branch document");
        let database = document_row(self.raw, "/databases", child)?;
        let proof = self.inventory.xml(database, child)?;
        proof.position(&database["position"], root)?;
        proof.attributes(database, root)?;
        let options = options::checked(root, database, proof, names, counts)?;
        let fragments =
            scopes::definition(self.archive, scopes::source_attribute(root, "FragDef"))?;
        let tags = scopes::definition(self.archive, scopes::source_attribute(root, "TagDef"))?;
        self.scopes(child, &fragments, Some(&tags))?;
        let bound = self.selectors(owner, &position, &options, names, &fragments, counts)?;
        let branch = candidates::Branch {
            parent: owner.actual,
            child,
            position: &position,
            actual: self.candidates,
            weapon_uses: self.weapon_uses,
            inventory: self.inventory,
        };
        counts.candidates += branch.compare(&options, &bound, &tags, attachment, motion)?;
        Ok(())
    }

    fn scopes(&self, entry: &Entry, fragments: &Scope, tags: Option<&Scope>) -> TestResult {
        let scope = unique(
            rows(self.raw, "/scopes")?.iter().filter(|row| {
                row.pointer("/database/entryIndex").and_then(Value::as_u64)
                    == u64::try_from(entry.index()).ok()
            }),
            "one scope per physical AnimDB",
        )?;
        for (field, original) in [("fragments", Some(fragments)), ("tags", tags)] {
            if let Some(original) = original {
                let coverage = scope
                    .get(field)
                    .and_then(|row| row.get("coverage"))
                    .ok_or("scope coverage")?;
                assert!(
                    coverage.pointer("/value").and_then(Value::as_bool) == Some(original.complete),
                    "scope coverage reflects the actual verified definition closure",
                );
                self.inventory.closure(coverage)?;
            }
        }
        Ok(())
    }

    fn selectors<'name>(
        &self,
        owner: &BranchOwner<'_, '_>,
        position: &Value,
        options: &[OptionRow<'_, '_>],
        names: &[&'name str],
        target: &Scope,
        counts: &mut Coverage,
    ) -> TestResult<Vec<&'name str>> {
        let parent = u64::try_from(owner.actual.index())?;
        let rows = rows(self.raw, "/fragmentSelectors")?
            .iter()
            .filter(|row| {
                row.pointer("/branch/value") == Some(position)
                    && row.pointer("/source/entryIndex").and_then(Value::as_u64) == Some(parent)
                    && text(row, "/selector/value").is_ok_and(is_reload)
            })
            .collect::<Vec<_>>();
        assert!(
            rows.len() == names.len(),
            "one outcome per physical reload selector"
        );
        let mut bound = Vec::new();
        for (row, &name) in rows.into_iter().zip(names) {
            counts.selectors += 1;
            assert!(
                text(row, "/selector/value")? == name,
                "exact authoring selector order"
            );
            let matching = options
                .iter()
                .filter(|option| option.group.eq_ignore_ascii_case(name))
                .collect::<Vec<_>>();
            check_positions(row, &matching)?;
            let status = selector_status(&owner.fragments, target, name, matching.is_empty());
            assert!(
                text(row, "/status/value")? == status,
                "original FragDef scope result"
            );
            self.inventory.closure(&row["status"])?;
            match status {
                "matched" => {
                    counts.matched_scope += 1;
                    bound.push(name);
                }
                "noMatchingGroup" => counts.no_group += 1,
                _ => counts.unresolved_scope += 1,
            }
        }
        Ok(bound)
    }
}

fn check_positions(row: &Value, options: &[&OptionRow<'_, '_>]) -> TestResult {
    let actual = rows(row, "/options")?;
    assert!(
        actual.len() == options.len(),
        "complete original match occurrences"
    );
    for (projected, option) in actual.iter().zip(options) {
        assert!(
            projected.get("value") == option.row.pointer("/position/value"),
            "selector links each original option in sibling order",
        );
    }
    Ok(())
}

fn selector_status(from: &Scope, to: &Scope, name: &str, empty: bool) -> &'static str {
    if empty {
        return "noMatchingGroup";
    }
    match (from.named(name), to.named(name)) {
        ([left], [right]) if from.complete && to.complete => {
            if left == right {
                "matched"
            } else {
                "differentScope"
            }
        }
        _ => "incompleteScope",
    }
}

fn original_branch(
    owner: &BranchOwner<'_, '_>,
    source: SourceElement<'_>,
    position: &Value,
    ordinals: [usize; 2],
) -> TestResult {
    let branch = unique(
        rows(owner.exported, "/branches")?
            .iter()
            .filter(|row| row.pointer("/position/value") == Some(position)),
        "exact original parent branch occurrence",
    )?;
    assert!(
        branch.pointer("/ordinals/value") == Some(&json!(ordinals)),
        "parent branch uses the original stored positions",
    );
    owner.proof.attributes(branch, source)?;
    let original = source
        .elements()
        .enumerate()
        .filter(|(_, element)| named(*element, "FragmentID"));
    let actual = rows(branch, "/fragments")?;
    let mut checked = 0;
    for (ordinal, selector) in original {
        let row = actual.get(checked).ok_or("original FragmentID row")?;
        owner.proof.position(&row["position"], selector)?;
        owner.proof.name(&row["name"], selector)?;
        owner.proof.attributes(row, selector)?;
        assert!(
            row.pointer("/ordinal/value") == Some(&json!(ordinal)),
            "original FragmentID sibling ordinal",
        );
        checked += 1;
    }
    assert!(
        checked == actual.len(),
        "all original FragmentID selectors survive"
    );
    Ok(())
}

fn selectors(branch: SourceElement<'_>) -> Vec<&str> {
    scopes::children(branch, "FragmentID")
        .filter_map(|selector| scopes::source_attribute(selector, "Name"))
        .filter(|name| is_reload(name))
        .collect()
}

fn is_reload(name: &str) -> bool {
    name.as_bytes()
        .windows("reload".len())
        .any(|part| part.eq_ignore_ascii_case(b"reload"))
}

fn named(node: SourceElement<'_>, name: &str) -> bool {
    node.namespace().is_none() && node.name() == name
}

fn parse(bytes: &[u8]) -> TestResult<SourceDocument<'_>> {
    SourceDocument::parse(bytes, SourceReadOptions::default())
        .map_err(|_| "parse verified original XML".into())
}

fn document_row<'catalog>(
    table: &'catalog Value,
    path: &str,
    entry: &Entry,
) -> TestResult<&'catalog Value> {
    let expected = u64::try_from(entry.index())?;
    unique(
        rows(table, path)?.iter().filter(|row| {
            row.pointer("/source/entryIndex").and_then(Value::as_u64) == Some(expected)
        }),
        "one physical XML document per Archive Entry",
    )
}
