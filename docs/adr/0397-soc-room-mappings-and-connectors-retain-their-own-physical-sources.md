# SOC room mappings and connectors retain their own physical sources

A SOCPAK can store a room registry, a RoomMapping document and an independent
room-connector document. Record each companion at the authored container
occurrence using its exact member path and CRC-checked payload. The companion
paths identify distinct sources even when XML documents refer to the same
entity.

The scene's ordered XML documents preserve RoomMapping declarations, exact
attribute values and physical positions. The published typed connector exposes
the stored coordinate arrays and joins its owner GUID only within a complete
census of the same container's declared SOC streams. Repeated owners remain
individually addressable. A known ambiguity remains explicit even if other
stream evidence is unreadable. The layout and spelling of compound RoomMapping
identifiers remain available for later interpretation.

The optional companion block is additive to schema v1. The same archive read,
CRC, cancellation and raw extraction paths apply to these members.
