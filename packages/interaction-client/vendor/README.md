# Offline audit dependencies

These exact npm package tarballs are committed to the audit source so an
isolated reviewer can run `npm ci --offline`, `npm test`, and
`npm run typecheck` without a registry connection.

- `typescript-5.9.3.tgz`
- `types-node-22.19.19.tgz`
- `undici-types-6.21.0.tgz`

The root `package-lock.json` records their SHA-512 integrity values.
