// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_cryxml::{self as cryxml, ByteOrder, XmlChildAbsence, XmlRawKind, XmlRenderSpan};
use p4kx_testkit::{CryXmlByteOrder, CryXmlSignature, TestElement};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[test]
fn leaf_markup_proves_absence_with_the_original_later_parent_words() -> TestResult {
    for (signature, order, directory, expected_order) in [
        (
            CryXmlSignature::CryXmlB,
            CryXmlByteOrder::Little,
            8,
            ByteOrder::Little,
        ),
        (
            CryXmlSignature::Cry3Sdk,
            CryXmlByteOrder::Big,
            9,
            ByteOrder::Big,
        ),
    ] {
        let xml = TestElement::new("root")
            .child(TestElement::new("empty"))
            .child(TestElement::new("inline").content("first text"))
            .child(TestElement::new("branch").child(TestElement::new("last").content("last text")));
        let mut input = xml.to_cryxml(signature, order);
        let table = usize::try_from(word_at(&input, directory + 4, order)?)?;
        input
            .get_mut(table + 28 + 10..table + 28 + 12)
            .ok_or("original ignored child count")?
            .fill(0xff);
        input
            .get_mut(table + 28 + 20..table + 28 + 24)
            .ok_or("original ignored first child index")?
            .fill(0xff);
        let rendered = render(&input)?;
        let first = leaf(&rendered, 1, b"/>\n")?;
        let inline = leaf(&rendered, 2, b"</")?;
        let last = leaf(&rendered, 4, b"</")?;
        assert!(
            first
                .sources()
                .any(|source| source.kind() == XmlRawKind::StringTerminator),
            "the empty text decision still retains the original terminator"
        );
        check_absence(first, &input, directory, order, expected_order, &[0, 0, 3])?;
        check_absence(inline, &input, directory, order, expected_order, &[0, 3])?;
        check_absence(last, &input, directory, order, expected_order, &[])?;
        check_positive_child(&rendered, table)?;
    }
    Ok(())
}

fn check_absence(
    span: &XmlRenderSpan,
    input: &[u8],
    directory: usize,
    order: CryXmlByteOrder,
    expected_order: ByteOrder,
    parents: &[u32],
) -> TestResult {
    let proof = span
        .child_absence()
        .ok_or("structured child-absence proof")?;
    assert!(
        !span.complete(),
        "contiguous-only consumers still see partial coverage"
    );
    assert_eq!(
        proof.byte_order(),
        expected_order,
        "original directory byte order"
    );
    assert_eq!(
        proof.node_directory(),
        &(directory as u64..directory as u64 + 12),
        "the original length, node offset and count words cover the directory"
    );
    assert_eq!(
        word_at(input, directory, order)? as usize,
        input.len(),
        "original file-size word"
    );
    let table = u64::from(word_at(input, directory + 4, order)?);
    let count = word_at(input, directory + 8, order)?;
    assert_eq!(count, 5, "original Nodes count");
    assert_eq!(
        count - proof.node_index() - 1,
        u32::try_from(parents.len())?,
        "only node rows following this leaf can name it as parent"
    );
    if parents.is_empty() {
        assert!(
            proof.parent_words().is_none(),
            "the last node needs only the table boundary"
        );
        return Ok(());
    }
    check_parent_words(proof, table, input, order, parents)
}

fn check_parent_words(
    proof: &XmlChildAbsence,
    table: u64,
    input: &[u8],
    order: CryXmlByteOrder,
    parents: &[u32],
) -> TestResult {
    let words = proof.parent_words().ok_or("compact parent-word suffix")?;
    let first = table + u64::from(proof.node_index() + 1) * 28 + 12;
    assert_eq!(
        words.first_offset(),
        first,
        "the word in the next node-table row"
    );
    assert_eq!(
        words.word_count(),
        u32::try_from(parents.len())?,
        "the full relevant suffix is surveyed"
    );
    assert_eq!(words.stride(), 28, "one word per physical node row");
    assert_eq!(
        words.word_width(),
        4,
        "only the actual original parent word contributes"
    );
    for (index, &expected) in parents.iter().enumerate() {
        let at = usize::try_from(first + u64::try_from(index)? * words.stride())?;
        let parent = word_at(input, at, order)?;
        assert_eq!(parent, expected, "original parent of later row {index}");
        assert_ne!(
            parent,
            proof.node_index(),
            "the row is not a child of this leaf"
        );
    }
    Ok(())
}

fn check_positive_child(rendered: &Rendered, table: usize) -> TestResult {
    let branch = rendered
        .spans
        .iter()
        .find(|span| {
            span.complete()
                && bytes(&rendered.xml, span) == Some(b"</")
                && usize::try_from(span.range().end)
                    .ok()
                    .and_then(|end| rendered.xml.get(end..))
                    .is_some_and(|tail| tail.starts_with(b"branch>\n"))
        })
        .ok_or("complete branch closure")?;
    assert!(branch.child_absence().is_none(), "the child exists");
    let mut found = branch.sources();
    let parent = found.next().ok_or("original first-child edge")?;
    assert_eq!(
        parent.kind(),
        XmlRawKind::NodeParent,
        "positive child proof uses its parent word"
    );
    let offset = u64::try_from(table)? + 4 * 28 + 12;
    assert_eq!(
        parent.range(),
        &(offset..offset + 4),
        "positive child proof is one original word"
    );
    assert!(found.next().is_none(), "one positive witness suffices");
    Ok(())
}

struct Rendered {
    xml: Vec<u8>,
    spans: Vec<XmlRenderSpan>,
}

fn render(input: &[u8]) -> TestResult<Rendered> {
    let document = cryxml::parse(input)?;
    let mut legacy = Vec::new();
    document.write_xml(&mut legacy)?;
    let mut rendered = Rendered {
        xml: Vec::new(),
        spans: Vec::new(),
    };
    document.write_xml_positioned(&mut rendered.xml, |span| {
        rendered.spans.push(span);
        Ok(())
    })?;
    assert_eq!(
        rendered.xml, legacy,
        "positioned and legacy bytes remain equal"
    );
    Ok(rendered)
}

fn leaf<'output>(
    rendered: &'output Rendered,
    node: u32,
    output: &[u8],
) -> TestResult<&'output XmlRenderSpan> {
    let mut matching = rendered.spans.iter().filter(|span| {
        span.child_absence()
            .is_some_and(|proof| proof.node_index() == node)
            && bytes(&rendered.xml, span) == Some(output)
    });
    let span = matching.next().ok_or("expected child-absence rendering")?;
    assert!(
        matching.next().is_none(),
        "one proof for each leaf decision"
    );
    Ok(span)
}

fn bytes<'input>(xml: &'input [u8], span: &XmlRenderSpan) -> Option<&'input [u8]> {
    xml.get(usize::try_from(span.range().start).ok()?..usize::try_from(span.range().end).ok()?)
}

fn word_at(input: &[u8], at: usize, order: CryXmlByteOrder) -> TestResult<u32> {
    let bytes = <[u8; 4]>::try_from(input.get(at..at + 4).ok_or("original word")?)?;
    Ok(match order {
        CryXmlByteOrder::Little => u32::from_le_bytes(bytes),
        CryXmlByteOrder::Big => u32::from_be_bytes(bytes),
    })
}
