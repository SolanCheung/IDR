# Remaining risks and release blockers

`PRODUCTION TRUST ROOT = BLOCKED`

Round 11 P0-01 has a Round 12 code remediation: Runtime has zero authority
base-table DML and every view write requires a transaction-bound Orchestrator
HMAC. The Round 12 audit then found a TEMP-table shadow path in inherited
`SECURITY DEFINER` resolution. Round 13 pins the trusted path, introduces the
fixed-work v8 opener, revokes v7 from Runtime and rejects TEMP/extra-role
Production logins. Neither remediation is declared independently closed until
a new auditor reproduces the exact attacks with separate deployment
identities. P0-00 still requires
confirmed revocation and rotation of the exposed `coding-plan` credential at
the provider; deleting local copies and excluding credentials from this
package does not revoke it.

Round 13 introduced no new peer-level Runtime defect, but its ZIP could not run
the packaged validator because of an `Aegis Life`/`Aegis-Life` path mismatch.
Round 14 corrected the path mismatch and is frozen as the final V1.3 audit
baseline. The final archive passed the clean-room validation entry, but a
future independent environment may still reproduce that evidence. Baseline
freeze does not close the deployment blockers below or authorize Production.

The following deployment or larger-scope P1 items remain open:

1. No production WORM/KMS-backed audit anchor or protected checkpoint signer is
   shipped.
2. Trust Root configuration and rotation are not backed by a protected
   distribution/control service.
3. Trust Root snapshot/rotation is not coordinated transactionally with the
   database commit. A protected provider must define serializable rotation and
   revocation semantics.
4. Checkpoint publication occurs after the database commit. Startup now
   verifies and republishes a missing tail before serving, but a production
   atomic publication protocol and failure-injection evidence are still absent.
5. The durable internal permit is not a provider-facing signed capability.
6. Response delivery and fan-out receipts are not implemented.
7. Historical Trust Root retention and full historical Proof signature replay
   remain incomplete.
8. Capability/Authority/Policy registries and issuer separation are incomplete.
9. Outcome conflict, attribution and observation aggregation remain minimal.
10. Human Model sensitive-inference classification, retention and decay jobs are
   incomplete.
11. Migration compatibility/replay corpus and long-running fuzz evidence have
   not reached a production release threshold.
12. The Aegis production adapter and production deployment remain compile
    blocked; two consecutive clean independent reviews have not occurred.
13. The repository still lacks Git provenance for this review snapshot.
14. The HMAC attestation key is locally file-provisioned; production KMS/HSM
    lifecycle, dual-control rotation and disaster recovery evidence are absent.

The production startup latch must remain blocked while any item above is open.

```text
IDR_TRUST_CHAIN_CLOSURE_COMPLETE = NO
PRODUCTION TRUST ROOT = BLOCKED
PRODUCTION AUTHORIZATION = NOT AUTHORIZED
PRODUCTION EXECUTION = NOT AUTHORIZED
HUMAN MODEL LONG-TERM WRITE = NOT AUTHORIZED
AEGIS PRODUCTION ADAPTER = NOT AUTHORIZED
```
