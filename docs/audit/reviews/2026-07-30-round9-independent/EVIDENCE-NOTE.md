# Round 9 independent evidence preservation note

The auditor-provided `SHA256SUMS` includes a digest for itself, so its
self-entry cannot validate after the file is finalized. That file and every
other original evidence entry are preserved byte-for-byte as received. Every
non-self entry validates. `OUTER-ZIP.sha256` records the independently verified
outer archive digest.
