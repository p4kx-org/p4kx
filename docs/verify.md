# Verifying Entry checksums

`p4kx verify <archive> [--filter <filter>]` reads every Entry (or those the Filter matches),
decrypts and decompresses it, and compares the CRC-32C of the decoded bytes with the one the
Archive directory states. An Entry whose directory also states a SHA-256 is checked against it
too (see below). `p4kx extract --verify` does the same for each Entry it writes: an
Entry that does not match is not put in place and is counted as a checksum mismatch.
`p4kx socpak verify` and `socpak extract --verify` check a container against the plain CRC-32 of
its directory. Reading is unchecked unless one of these is asked for (ADR-0026).

An Entry that the Skip Conflict policy leaves alone because its file exists is never read, so it is
not verified; under IfChanged an unchanged file matched the stored CRC-32C when it was compared.
A texture exported as PNG or EXR reads its header Entry to the end through the verified reader, so
a mismatch there is reported although the Conversion parses only part of it.

## The SHA-256

Every Entry of the Archive measured (LIVE 4.10) states a 32-byte
hash, in the `0x5003` extra block of its v1 directory record. It is the SHA-256 of the **stored
bytes** of the Entry, the `compressed_size` bytes as they lie in the file: for an encrypted Entry
the ciphertext with its padding, for a compressed one the compressed data. It is not the SHA-256
of the decoded data (ADR-0034). So the SHA-256 needs no Archive key, and a wrong key fails the
CRC-32C but not the SHA-256.

The check is on whenever verification is: it needs no switch of its own, and a mismatch is reported
like a CRC mismatch with the kind `sha256` and the digests as 64 hex digits. The stored bytes are
hashed as the reader consumes them; the bytes it leaves unread (AES padding, a compression
trailer) are read and hashed when the decoded data ends, unless the CRC-32C already failed: then
the CRC mismatch is reported and those bytes are not read, so a failing read cannot replace it.
An all-zero digest, as the synthetic fixtures use, states none and is not checked. A `0x5003`
block that is not 36 bytes long states none too and is an Entry issue (ADR-0148); of several
blocks in one record the last decides.

The v2 record has 32 bytes after the encryption field. They are read and returned by
`Entry::sha256`, not checked: no v2 Archive was available to confirm that they are the same
digest, so only the v1 SHA-256 is verified.

The `sha2` crate uses the SHA-NI or ARMv8 SHA-2 instructions where the CPU has them and a portable
implementation elsewhere.

## How the checksums are computed

The checksum is computed on the chunks that leave the reader, so a run reads and decodes the
Archive once, holds at most one 1 MiB chunk per worker and writes nothing. The CRC-32C comes from the
`crc32c` crate (hardware instructions on x86-64 and AArch64, portable fallback), the CRC-32 from
`crc32fast`.

## Exit codes and output

| Result | Exit code |
|--------|-----------|
| every Entry matched | 0 |
| a mismatch, or an Entry that cannot be read | 1 |
| a Filter that matches no Entry | 2 |
| the Archive cannot be opened | 3 |
| the Archive changed on disk during the run | 4 |

Text output prints `mismatch <path>: <computed> computed, <stored> stored` and
`failed <path>: <reason>` to stderr, then a summary line on stdout. `--json` prints the document
described in [cli-json.md](cli-json.md#p4kx-verify-archive---filter-filter---json).

## Run time

| Archive | Entries | Size | Machine | `p4kx verify` | Result |
|---------|---------|------|---------|---------------|--------|
| LIVE `Data.p4k` | 1,379,308 | 162 GB (file size of the Archive) | Ryzen 9 9950X3D, 16 cores, 32 threads | 50 to 57 s (three runs, release build) | 0 mismatches, 0 failed |

For scale: a review of an external extractor that checked only sizes measured about 27 minutes for
a full scan of LIVE 4.10 (1,362,047 Entries). The runs above were bound by the disk at about
3 GB/s: the 150.9 GiB Archive does not fit into the 94 GiB of RAM, so the file cache could not hold
it between runs.

### What the SHA-256 costs

Measured on LIVE with the file in the cache, so that the disk does not hide the CPU cost
(release build, same machine); the benchmarks are `bench_verification_cost` in `p4kx-archive` and
`bench_verify` in `p4kx-extraction` (ADR-0137, ADR-0148):

One thread, 300 Entries of 64 KiB to 16 MiB (176 MiB), median of five runs after a warm-up, all
three in one process (`bench_verification_cost`):

| Read | Time |
|------|------|
| without a check | 332 ms |
| CRC-32C | 351 ms |
| CRC-32C and SHA-256 (`open_entry_verified`) | 395 ms (+13 % over the CRC-32C) |

All cores, `verify` of 76,310 Entries (8,225 MiB), each figure the median of three runs after a
warm-up in one process (`bench_verify`). The "without" figure comes from a build in which the
stored-bytes check was switched off locally (one process, not committed); the other two are
separate processes of the committed build:

| Build | Time |
|-------|------|
| without the SHA-256 check | 2.03 s |
| with the SHA-256 check | 1.88 s and 2.14 s |

The SHA-256 adds about 0.25 ms per MiB on one core. With all cores the difference lies within the
spread between processes, so "no separate switch" (ADR-0034) stays.

## Real-Archive tests

Use the [real-Archive setup](../CLAUDE.md#real-test-data) and select the relevant check.

- `p4kx-archive` verifies a sample of each compression and encryption kind with
  `Archive::open_entry_verified` (CRC-32C and SHA-256). This check needs the configured Archive key.
- `p4kx-archive` checks that every Entry states a SHA-256 digest and compares it with the bytes
  of cleartext Entries while streaming.
- `p4kx-extraction` runs `verify` over every 997th Entry.

```powershell
cargo test --release --locked -p p4kx-archive --test real_archive -- archive::verified_reads_of_every_compression_and_encryption_state_find_no_mismatch --ignored --exact
cargo test --release --locked -p p4kx-archive --test real_archive -- archive::every_entry_states_a_sha_256_that_plain_entries_match_while_streaming --ignored --exact
cargo test --release --locked -p p4kx-extraction --test real_archive -- verify::a_sample_of_a_real_archive_verifies_without_a_mismatch --ignored --exact
```
