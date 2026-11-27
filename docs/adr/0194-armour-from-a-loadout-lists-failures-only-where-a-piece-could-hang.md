# Armour from a Loadout lists failures only where a piece could hang

Decided for issue #517, the follow-ups of the review of ADR-0082 (armour from a Loadout). Evidence:
the `TEST_Ranger` character of the testkit and `crates/p4kx/tests/it/armour_loadout.rs`.

## Where a failure counts

An Entity is worn when its `AttachDef.Type` is a worn item type (ADR-0082); the root of the tree
counts as worn. A failure is listed as `piece_not_found` when it sits at the root, on a worn Entity
or on the ports below them, including the nested Loadout of an empty port:

- an unresolved attachment on such a port;
- the Loadout of the root that could not be read in full, such as an XML include the Archive
  lacks, as a failed piece named by the root.

The Loadout of a worn Entity that could not be read in full does not make its piece fail: the
piece is fitted, and the issue is listed as `loadout_unreadable` under the piece's name. It leaves
nothing out, so it does not count in `skipped`. A `--piece` that replaces the Entity takes the
issue with it. Issues that only say a class name is ambiguous or that a vehicle implementation
cannot be read are left out, as for the root.

Nothing below an Entity that is not worn is a failure: an optic the Archive lacks on a rifle is no
armour. A worn Entity that hangs below a non-worn one is still found and worn, and failures below
it count again. A head is a non-worn Entity here. Its parts belong to `NpcHead` (ADR-0109).

The reason: the failures exist so that the report does not hide a piece that was meant to be worn.
Below a weapon or a body part nothing is meant to be worn, and its unresolved attachments are no
failed armour piece.

## Names

Every piece, every failure of an attachment and every `loadout_unreadable` issue of a Loadout tree
is named `<port>/<class name>`, with the name of the port the Entity hangs on, not the path of
ports above it. An unresolved attachment that names
a Reference instead of a class name is `<port>/<GUID>`, and one whose Reference cannot be read is
`<port>/unreadable`. The root's own failures are named by its record path or, for an XML Loadout,
its path.

## A failed piece and `--piece`

A `--piece` takes the place of a Loadout piece of the same item type (ADR-0082). A failed piece
has no item type: the type is part of the Record that could not be found or read, and the port
that names it does not fix it, because a port takes several types. A failed Loadout piece
therefore cannot be replaced by a `--piece`; it stays listed as skipped, and the `--piece` is
added to the others.
