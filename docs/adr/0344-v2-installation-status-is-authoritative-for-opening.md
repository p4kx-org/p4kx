# v2 installation status gates Archive opening

## Decision

Refuse a v2 Archive whose known incomplete flag is set, even if selected payloads would pass
integrity verification. There is no v2 partial-install recovery mode. Preserve the raw status byte
and unknown bits without assigning them semantics. Expose the available declaration through the
shared Archive API; v1 absence is unavailable rather than confirmed completeness.

Retain installed-byte counters after checking their ranges and bounds against stored lengths.
Counters describe source installation metadata and remain distinct from verified payload integrity.
Normal frontends cannot bypass the same Archive-open failure or publish from that source.

## Consequences

No per-frontend recovery policy is needed. Metadata export can reuse typed status and counter
accessors. Synthetic builders write complete install blocks and the same tests cover top-level
Archives and container admission. See [the installation contract](../archive-installation-status.md).
