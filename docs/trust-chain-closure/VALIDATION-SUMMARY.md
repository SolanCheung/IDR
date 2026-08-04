# IDR V1.3 Round 14 validation summary

Validation date: 2026-07-31  
Source-tree raw output: `idr-v13-round14-validation-output.txt`  
Raw output SHA-256:
`29a655efc63d03fae7570f580c786d2ae03a2074579756cad6b37571adde6ce8`

## Environment

| Tool | Version |
| --- | --- |
| Rust | rustc 1.94.1 / cargo 1.94.1 |
| Node/npm | 22.16.0 / 10.9.2 |
| Python | 3.9.6 |
| PostgreSQL client/server test target | psql 16.9 / local PostgreSQL 16.9 |

## Results

| Validation group | Result |
| --- | --- |
| Immutable Round 11 evidence SHA-256 | PASS, 6/6 entries |
| Supplied Round 12 evidence SHA-256 | PASS, 5/5 entries |
| Archived Round 13 assessment SHA-256 | PASS, 1/1 entry |
| Aegis root resolver against source checkout | PASS, `Aegis Life` |
| Aegis root resolver against packaged layout | PASS, `Aegis-Life` |
| Invalid explicit Aegis override | PASS, rejected closed |
| Rust default workspace tests | PASS |
| Rust Shadow PostgreSQL integration/attack tests | PASS, 5/5 |
| Rust Production release-gate tests | PASS, 17/17 |
| rustfmt | PASS |
| strict Clippy default/Shadow/Production/Migration/migrator | PASS, zero warnings |
| Production and Migration-only builds | PASS |
| forbidden feature combinations and Production test API | expected compile failures observed |
| separate Production migrator | PASS, schema version 8 |
| Runtime real-table DML / authority views | PASS, zero base tables writable; 22 views |
| Runtime HMAC key / TEMP | both DENIED |
| Runtime role graph | exact MEMBER/SET/USAGE PASS; unexpected SET roles 0 |
| SECURITY DEFINER paths | PASS, 4/4 pinned with `pg_temp` last |
| attestation opener ACL | PASS, v8 only; v7 denied |
| TEMP key/attestation shadow exploit | PASS, attacker key and matching fake MAC rejected |
| fixed-work HMAC validation | PASS, canonical 32-byte comparison path |
| raw Runtime Run/Contract/Receipt/Fence/attestation attacks | PASS, all rejected |
| bound Policy revocation after Admission | PASS, persistent invalidation and governed event |
| Receipt content HMAC/replay/startup binding | PASS; privileged mutation detected |
| secret scanner self-test and source scan | PASS, zero findings |
| TypeScript clean install/tests/typecheck | PASS, 12/12 and zero type errors |
| Python evaluation/conformance | PASS, 9/9 |
| generated cross-language artifact drift | PASS, zero drift |
| Aegis IDR Shadow adapter and full workspace | PASS |
| Aegis Production adapter gate | expected compile failure observed |
| dependency direction | PASS |

Final script marker:

```text
ALL_ROUND14_VALIDATION_STEPS=PASS
```

## Scope of evidence

This validates the source-defined version-8 boundary and reproduces the exact
class of TEMP shadowing attack with attacker-controlled key material. It does
not prove a real deployment identity, provider credential revocation, KMS/HSM
key lifecycle, WORM checkpoint controls or protected Trust Root distribution.

The final archive is validated only after it is immutable. Its independent
empty-directory extraction log and SHA-256 sidecar are therefore delivered
next to, rather than embedded inside, the ZIP. A valid delivery must end with
both `ALL_ROUND14_VALIDATION_STEPS=PASS` and
`ALL_ROUND14_CLEAN_ROOM_STEPS=PASS`.

Round 14 changes audit reproducibility only. Evidence Qualification, Setoka,
bitemporal querying and other semantic or authority changes remain deferred to
the separately scoped IDR V1.4 architecture delta.

The Production release latch remains unchanged:

```text
IDR_TRUST_CHAIN_CLOSURE_COMPLETE = NO
PRODUCTION TRUST ROOT = BLOCKED
PRODUCTION AUTHORIZATION = NOT AUTHORIZED
PRODUCTION EXECUTION = NOT AUTHORIZED
HUMAN MODEL LONG-TERM WRITE = NOT AUTHORIZED
AEGIS PRODUCTION ADAPTER = NOT AUTHORIZED
```
