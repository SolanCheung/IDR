# Round 12 independent audit evidence

These files are the exact evidence supplied after the Round 12 package was
reviewed on 2026-07-30. `SHA256SUMS` was generated only after the five supplied
files were copied into this immutable review directory.

The independent executable checks passed for package integrity, TypeScript,
Python and secret scanning. The static trust-boundary review identified a new
P0 exploit: a Runtime login with PostgreSQL TEMP privilege could place
attacker-controlled tables ahead of the inherited `SECURITY DEFINER`
`search_path`.

Round 13 treats that exploit as a real implementation defect. See
`../../../trust-chain-closure/ROUND13-REAUDIT-RESPONSE.md`.

