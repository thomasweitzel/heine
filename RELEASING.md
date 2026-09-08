# Releasing Heine

This procedure keeps a pre-release project ready for a deliberate public
release without treating a local check as a substitute for later independent
automation.

## Everyday changes

Before a change is considered complete, run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
```

Record a user-visible change in `CHANGELOG.md` under `Unreleased` as part of
the same change. Follow its maintenance rules, including a smallest complete
migration for incompatible behavior.

## Preparing a release

1. Begin from a clean, up-to-date release branch. Review `Unreleased` for
   omissions and set the intended version in `Cargo.toml`.
2. Move the unchanged `Unreleased` entries under
   `## [<version>] - YYYY-MM-DD`, then create a new empty `Unreleased` section.
3. Run the everyday checks above, followed by:

   ```bash
   cargo build --release
   cargo package
   cargo package --list
   ./target/release/heine --help
   ./target/release/heine --root starter-site
   ```

   `cargo package` checks the crate archive Cargo would publish. Inspect the
   listed files to ensure it contains the intended source, documentation, and
   license material, and no local-only material.
4. Before the first crates.io publication, add a public canonical
   `repository` URL to `Cargo.toml`, confirm the package description,
   categories, keywords, and Rust version remain accurate, and run
   `cargo publish --dry-run` with the intended registry configuration.
5. Commit the release preparation and push the intended release branch. Then
   create a signed annotated tag and verify it before pushing the tag:

   ```bash
   git push
   git tag -s v<version> -m "Heine <version>"
   git verify-tag v<version>
   git push origin v<version>
   ```

   The tag is deliberately signed even when commits are signed automatically.
   It identifies the reviewed source revision from which release artifacts are
   built.

Dependency advisory, provenance, duplicate-version, and license-policy
automation remain future work. Do not present their absence as a clean bill of
health.
