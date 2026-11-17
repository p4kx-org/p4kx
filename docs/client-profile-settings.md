# Client profile settings

`p4kx actionmap settings <archive>` writes a JSON report to stdout.
The Rust SDK uses `read_client_profile_settings` for both documents and
`read_client_profile_settings_for` with a `ProfileKind` for a single payload.
Both calls need the opened Archive directory and selected profile XML. Settings,
physical candidates, elements, declarations and text are accessible as typed values.
The report also provides the checked Archive's Build when available and its own
source inventory for resolving the returned origin references.
Catalog schema v3 exposes the same documents under `configurations.clientProfileSettings`.
The CLI report wraps them in `settings` and includes its own `provenance` inventory.
Catalog references use the Catalog's shared inventory.

The `documents` array places the general client profile first and the VR client profile second.
A single-profile SDK call retains both selection outcomes and marks the sibling
`filtered` using directory evidence. An ambiguous profile lists every physical
candidate and exposes no decoded elements.
Each document records its requested path and the physically matched directory candidates.
The `status` cell distinguishes `read`, `missing`, `filtered`, `unavailable`, `ambiguous`,
`unreadable`, `malformed` and `unsupported`. A normal Catalog uses its existing publication
policy for unreadable or unproved inputs. The CLI prints the structured report and exits with
code 1 for a failed input. A profile absent from a complete directory is recorded as `missing`.

Each successfully decoded `Attributes` element tree is written as `elements` in document order.
A `parent` value is the zero-based position of the parent element in this document, and the root
has a null parent value. The `position` carries the original XML byte range or CryXml node index.
Each element contains its ordered attribute and namespace `declarations`. Prefixed names retain
their original spelling. Namespace URIs remain in their own authored declarations. An `Attr`
element typically declares `name`, `value` and an optional `cvar`. Additional declarations and child
elements use the same representation, including repeated settings and explicit empty strings.

The `value` cell retains the decoded string. A `lexeme` cell is present when a text XML
spelling differs from that string. Element `content` carries direct text values and their
original positions. For CryXml, the location addresses the original content or attribute
table slot. Each value cites its actual Archive source and carries
`sourceUsesRefs`, `sourceSetComplete` and `containsEncryptedSource`.

These documents describe the defaults authored in shipping client profiles. Effective
values, user overrides, enum-to-label mappings and persistence require independent
evidence. The ActionMap continues to carry bindings. When an action has a
`UIDescription`, its JSON row includes the separate `ui_description` source value.

[Profile source decision](adr/0398-client-profile-defaults-retain-original-xml-positions.md)
