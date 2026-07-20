# Heimlern compatibility freeze

HausKI retains two minimal local compatibility crates for the optional `heimlern` feature of
`hauski-policy-api`. They do not fetch, execute or import the historical
`heimgewebe/heimlern` repository.

The authoritative machine-readable boundary is
`docs/contracts/heimlern-compatibility-freeze.v1.json`.
`scripts/verify_heimlern_freeze.py` validates it through `scripts/check-vendor.sh`, which is already
called by the normal build, lint, test, release, coverage and security paths.

The freeze guarantees only local build compatibility. It does not establish an active Heimlern
service, learning authority, automatic policy application, semantic equivalence with the complete
historical repository or permission to expand the shim without separate review.

The deprecated optional workflow for direct Außensensor input was removed because its declared
`export/feed.jsonl` input did not exist and the supported event contract is owned outside this
compatibility boundary.
