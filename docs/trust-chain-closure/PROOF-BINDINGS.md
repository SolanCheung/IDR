# IDR V1.3 production proof binding table

Every production proof binds:

- proof ID/type, issuer, key ID and Ed25519 signature;
- signed `trust_domain` and `environment_ref`;
- exact subject reference and canonical subject digest;
- domain/environment-specific audience, tenant, scope, purpose and policy revision;
- issued/not-before/expires window and unique nonce;
- actor/caller `principal_digest`;
- typed assertion payload;
- current Trust Root version/digest at consumption.

Every row below additionally requires `CallerAuthentication` with
`outcome=authenticate`; it is not a fallback used only when no specialized
proof exists.

| Command boundary | Command-specific proof kinds | Required assertion |
| --- | --- | --- |
| Start input | InputAdmission | `outcome=admit` |
| Context | ContextSnapshot | `outcome=attest` |
| Intent | IntentFastPathAdmission | `outcome=admit` |
| Decision | DecisionNecessityAdmission | `outcome=admit` |
| Turn | TurnCoordinationAdmission | `outcome=admit` |
| Response send | ResponsePolicy + ResponseAdmission | `outcome=allow` |
| Action admission | Capability + Authority + Policy + ExactAuthorization + ActionAdmission | four `allow`; authorization `decision=approve` |
| Reserve/recover/deliver/start/expire | ExecutionPermit | `outcome=permit`; caller must be bound owner/provider |
| Provider receipt/reconciliation | ProviderReceipt | `outcome=attest` |
| Outcome | OutcomeObservation | `outcome=observe` |
| Human Model promotion | HumanModelPromotion plus conditional UserConfirmation or OutcomeObservation | `promote`, then `confirm` or `observe` |
| Human Model correction | HumanModelCorrection | `outcome=correct` |
| Human Model assertion write | HumanModelPromotion | `outcome=promote` |
| Cancel Run | Authority | `outcome=allow` |
| Commands with no specialized domain proof | none beyond CallerAuthentication | `outcome=authenticate` |

The Store requires the exact set: missing, duplicate or extra proof kinds fail.
Verification occurs after aggregate locking with PostgreSQL time and the
current Trust Root. Key revocation between precheck and consumption therefore
fails. Trust Root keys themselves are bound to the same trust domain,
environment and audience; a Shadow proof cannot be accepted under a Production
expected identity, nor can one environment's proof be redirected to another.
Exact Receipt replay revalidates the same CallerAuthentication and specialized
proof set before returning stored data.

Action Admission persists the exact proof ID and expiry for Capability,
Authority, Policy, ExactAuthorization and ActionAdmission. Reserve, recover,
delivery and dispatch load all five immutable proof envelopes and reverify
their kind, issuer policy, signature, subject/context assertion, validity and
revocation against trusted database time and the current Trust Root. Action,
Admission, every bound proof, Permit and lease validity are all checked before
execution authority advances.
