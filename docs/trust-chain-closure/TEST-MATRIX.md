# IDR V1.3 Round 14 test matrix

| Boundary | Positive and attack coverage |
| --- | --- |
| Authority API | downstream import/constructor/Deserialize/ref/proof/authority/direct-transition/removed-repository compile failures |
| Feature isolation | production + test-support/shadow-mode/dev-file-store/migration and shadow-mode + migration are expected compile failures |
| Trust identity | domain/environment signed into Command/Proof and bound to Trust Root/key/audience/public refs/receipts/checkpoints; foreign domain/environment rejected |
| Authority validity | every upstream current record checked at trusted time; expired Context/Intent/Decision/Turn/Response plus stale Action/Admission and overlong permit/lease rejected |
| Proof exact target | full unsigned command binding; Run/version/principal/correlation/causation and HM target redirections rejected |
| Command idempotency | every command requires CallerAuthentication plus semantic authority; exact replay revalidates current time/Trust Root and succeeds; revoked replay fails; same command ID with different canonical bytes returns `IdempotencyConflict` |
| Orchestrator attestation | exact txid/backend/command/Run/tenant/version/digest HMAC; canonical 32-byte fixed-work comparison; immutable nonce/key version; exact Receipt-content MAC; Receipt replay and startup verification |
| Stolen Runtime login | zero DML on all base tables and zero sequence privilege; raw Run/Contract/Receipt/Fence/attestation mutations rejected; HMAC key unreadable; TEMP-table key/attestation shadow exploit rejected |
| Function and login boundary | all SECURITY DEFINER paths pin trusted schema before explicit pg_temp; v8 sole Runtime opener; v7 denied; Production Runtime TEMP, extra MEMBER/SET, CREATE and owner authority fail closed |
| Admission revocation governance | Policy key revoked after Admission persists Admission invalidation and a governed cancellation event; post-Dispatch failure maps to ReconciliationRequired |
| Projection/audit | every Contract and execution projection replayed; specialized/current/proof state cross-checked; record-set and execution-state roots checkpointed; tamper and rollback fail closed |
| Aggregate lineage | subject and turn switching rejected in runtime; successor lineage duplicated by PostgreSQL trigger |
| Run terminal gate | Cancelled Run rejects reserve/delivery; terminal Receipt maps execution and Run state consistently |
| Decision/Action | selected option, real operation and parameter digest must exactly match; substitution rejected; DB dependency trigger duplicates the check |
| Successor invalidation | successor automatically loads and persists recursive reverse dependency closure; downstream promotion/assertion invalidated |
| Execution identity | owner-bound reserve/recovery/expiry; provider-bound delivery/start/receipt/reconciliation; unauthorized provider rejected |
| Exactly-once | append-only Fence owns the unique domain/environment/tenant/operation/idempotency scope; Reservation tamper cannot release it before restart; cancelled old Action does not release or reuse it |
| Receipt | exact Action-derived request digest, permit, provider, nonce and attempt; wrong request digest rejected in runtime and trigger |
| Outcome/Human Model | exact Outcome and Promotion lineage; explicit 86,400-second Receipt observation and 604,800-second Outcome cognitive-use windows; invalidated/late evidence rejected |
| Reconciliation | enum-only terminal resolution rejected; terminal reconciliation requires exact provider Receipt |
| Protocol | constrained types, unknown fields, candidate/rendered/idempotency resource limits, portable time range, UTF-16 JCS, safe integers, golden vectors and deterministic mutation fuzz |
| Response | rendered-byte tamper, policy/admission proof set and send nonce replay |
| PostgreSQL | separate environment-secret Production migrator, schema version 8, owner/base/view boundary, exact Runtime/read-only auditor ACLs, fresh replay preflight, CAS race, full proof revalidation, permanent Fence, privileged pre-restart duplicate attack and external checkpoint rollback/recovery |
| Package hygiene | explicit source allowlist, generated/dependency/credential path denial, symlink denial and redacted fingerprint-only secret scan |
| Final archive replay | shared source/package Aegis resolver; outer and inner hashes; empty-directory extraction; packaged full validator; external clean-room PASS log bound to immutable ZIP |
| Cross language | Rust/TypeScript/Python canonical bytes, digest, signature and unknown-field behavior |
| Aegis | shadow adapter candidate-only direction and production adapter compile block |

Local tests are remediation evidence, not an independent audit. Production
release still requires deployment-grade external dependencies and two
consecutive independent reviews with no trust-boundary P0.
