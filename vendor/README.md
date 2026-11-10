# Forked crates

Copies of crates.io releases with Impulse's changes, used through `[patch.crates-io]` in the root `Cargo.toml`. Examples, tests and benches are left out.

To update one, copy the new release over it and apply the changes again: `git diff` against the commit that added the release shows them.
