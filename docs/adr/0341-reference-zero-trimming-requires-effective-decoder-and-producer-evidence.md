# Reference zero trimming requires producer and effective-decoder evidence

Decided for issue #1105; narrows the byte allowance in ADR-0234.

## Evidence

The inspected unmodified unp4k revision
[b492ab14d26280c6ec91c4365ff0faf5f3e24a6b](https://github.com/dolkensp/unp4k/tree/b492ab14d26280c6ec91c4365ff0faf5f3e24a6b)
trims decrypted **encoded** bytes in
[CreateAndInitAesDecryptionStream](https://github.com/dolkensp/unp4k/blob/b492ab14d26280c6ec91c4365ff0faf5f3e24a6b/src/ICSharpCode.SharpZipLib/Zip/ZipFile.cs#L3116-L3140),
before [GetInputStream selects decompression](https://github.com/dolkensp/unp4k/blob/b492ab14d26280c6ec91c4365ff0faf5f3e24a6b/src/ICSharpCode.SharpZipLib/Zip/ZipFile.cs#L738-L795).
It does not justify loss from arbitrary decompressed output.

The trim loop stops at position one. Stored output can therefore retain one byte, including
zero, or at least three bytes ending in a nonzero byte. A two-byte result or a longer retained
prefix ending in zero is inconsistent with the loop. The loop can also drop a nonzero byte
at position one. That loss remains a difference.

After trimming, the method-100 branch reads up to four bytes from the decrypted memory stream.
Both outcomes of its positive-read branch seek back exactly four bytes. If the stream holds
one to three bytes, the rewind fails before the producer can return a stream to its caller.
A successful shorter method-100 pass-through reference therefore retains at least four bytes
and ends in a nonzero byte. The producer's
[Zstandard recognizer](https://github.com/dolkensp/unp4k/blob/b492ab14d26280c6ec91c4365ff0faf5f3e24a6b/src/Zstd.Net/Zstd.cs#L24-L32)
uses the same four-byte frame magic as the P4KX decoder.

## Decision

- A zero suffix is allowed only for an encrypted Entry with validated producer/source evidence
  for that exact unmodified producer revision. A missing context, another revision, any local
  patch scope or a mismatched Entry path/method/encryption identity fails closed.
- The actual decoder must be Stored for declared method 0 or PassThrough for CIG method 100.
  Deflate, actual Zstandard frames and an undetermined encoding never qualify. Method 100 alone
  is insufficient evidence.
- Compare both streams normally and retain their reference length and final reference byte.
  Missing bytes must be nonempty and all zero, and the retained prefix must match the exact
  boundary above. There is no invented maximum trim length.
- An allowed difference is separately counted under its producer-specific reason. It is never
  exact equality and never evidence of complete reference-run coverage. Manifest validation,
  producer success and complete selection accounting belong to the reference-run trust layer.
  An unmanifested comparison supplies no producer context; byte identity may still be established,
  but missing bytes cannot use the allowance.
- `EntryReader::effective_encoding` observes the reader's actual selected decoder without I/O,
  additional source reads or decoded-payload buffering. For method 100 it is Undetermined until
  the existing decoder reads its decrypted prefix; an empty logical Entry may remain
  Undetermined. The observation proves decoder selection, not a checksum or successful decoding.
  `EffectiveEncoding` is an additive read-only Archive API, re-exported by extraction.
- Decoder/extraction byte semantics, key handling, caller verification options and existing
  public reader methods remain unchanged. The decoder is a typed internal enum; it does not
  introduce shared mutable state, a second magic probe or a frontend dependency.

## Verification

Synthetic v1/v2, encrypted/plain Archive fixtures exercise the public observation while preserving
the full byte stream. Reference mutation tests cover real Deflate, real Zstandard, method-100
pass-through and Stored payloads with logical zero suffixes, the Stored one-byte minimum, the
method-100 four-byte rewind boundary, inconsistent
retained-zero/two-byte boundaries, source/producer mismatches, unknown contexts, nonzero loss,
changed/added bytes, multi-block suffixes and exact comparisons. Hosted tests use synthetic keys
and bytes only. Local independent-reference runs retain their matching Build/source evidence.
