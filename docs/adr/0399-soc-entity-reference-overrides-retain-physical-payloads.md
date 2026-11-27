# SOC entity-reference overrides retain physical payloads

A physical override member may be reused in several authored container
occurrences. SOC scene schema v1 stores every verified member payload once by
physical source reference. Its lossless Base64 supports byte replay even when
the XML parser cannot interpret the member. Ordered XML tables and typed
declarations let consumers address individual nodes and attributes without
reparsing those bytes.

Each occurrence keeps the members in archive order. Distinct physical members
remain separate when their paths are equal. A placed owner requires an exact
selector in a complete census of the occurrence's declared streams. The
selector, override index, Target and AA retain their authored spelling and source
positions. Payload and XML field provenance identify the original member. A
resolved owner also cites the matching SOC entity.
