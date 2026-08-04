# IDR V1.3 Round 7 response to the Round 6 independent review

Date: 2026-07-29

Status wording in this document is deliberately limited to “remediated in the
candidate.” Only an independent re-auditor can mark a finding closed.

| Finding | Round 7 remediation | Local regression |
| --- | --- | --- |
| P0-01 authority laundering | removed public repository trait and generic orchestrator; record, authority and transition engine are crate-private; concrete PostgreSQL orchestrator owns the only mutation path | compile-fail: removed repository, private authority, private transition and private record |
| P0-02 writable pool | production pool accessor removed; pool exists only as private state; narrow helpers are feature-gated for tests | production API/static visibility review |
| P0-03 raw projection | constrained projection digest stored, recomputed and included in latest audit payload; full audit chain/column replay added | projection JSON and audit-column tamper fail closed |
| P0-04 lineage switching | central subject/turn gate plus PostgreSQL lineage trigger | subject and turn substitution attacks rejected |
| P0-05 Action derivation | Decision stores selected option, operation and parameter digest; Action must match exactly; DB dependency trigger duplicates the invariant | selected-option substitution rejected |
| P0-06 successor invalidation | issuing a successor loads reverse dependency closure and persists invalidations atomically | Candidate successor invalidates dependent Promotion and Assertion |
| P0-07 terminal Run gate | central command-state gate executes before command logic | Cancelled reserve and delivery rejected |
| P0-08 lifecycle authority | owner/provider caller binding and specialized proofs for every execution lifecycle command | unauthorized provider delivery rejected |
| P0-09 operation idempotency | Reservation persists the real selected operation; unique tenant/operation/idempotency scope retained | stored real operation asserted; schema constraint inspected |
| P0-10 request digest | digest derives from exact Action ref, operation and parameter digest; carried through admission/reservation/receipt and DB trigger | substituted request digest rejected |
| P0-11 HM substitution | Assertion content is internally materialized from Candidate; request can set lifecycle/impact only; DB guard duplicates immutable content | added-predicate/content substitution rejected |
| P0-12 exact Outcome | Candidate and Promotion bind exact Outcome ID/revision/digest | wrong Outcome digest rejected |
| P0-13 command conflict | receipt replay requires tenant, Run, actor, caller and exact canonical command digest | same command ID/different bytes rejected |
| P0-14 unauthenticated controls | raw inspect, Human Model query and outbox controls removed from production build; test helpers require `test-support` | production visibility matrix/static check |

P1-01 through P1-10 remain release blockers and are reproduced without
downscoping in `REMAINING-RISKS.md`.

The old Round 5 closure matrix is retained only as explicitly superseded
historical evidence. The TypeScript “Authorization Submission” object is now
named an Authorization Candidate so product code cannot imply that it is Rust
authority.
