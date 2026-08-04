# IDR Interaction Client

TypeScript boundary for product and host integrations.

This package may:

- validate transport shapes;
- submit canonical input;
- display runtime decisions;
- submit user approval or denial bound to an exact action revision.

It must not:

- reproduce Rust guard rules;
- grant authority;
- promote Human Model assertions;
- infer that an action was executed;
- manufacture execution receipts.

## Offline audit

Exact TypeScript and Node type packages are included as local tarballs under
`vendor/`. Reviewers can run `npm ci --offline` with an empty npm cache before
running the tests and typecheck; no registry access is required.
