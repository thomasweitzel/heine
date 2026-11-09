# Releasing Heine

This procedure publishes a reviewed source release, platform archives on
GitHub, and the source package on crates.io. A signed tag identifies the
source used for both publications.

The maintainer decides that a release is ready and creates the signed tag.
Automation builds, checks, packages, and publishes the GitHub artifacts. The
maintainer prepares the source, creates the tag, and publishes to crates.io.

For an established public repository, follow these steps in order:

1. [Prepare and tag the source](#prepare-the-source-release).
2. [Inspect the GitHub release](#public-github-releases), its assets, and attestations.
3. [Publish to crates.io](#publish-to-cratesio).
4. [Update the documentation site](#update-the-documentation-site).

The initial setup, private rehearsal, workflow structure, and recovery sections
provide details when they apply.

## Initial repository setup

For a new repository that is still private, complete this setup before its
first public release:

1. Review the Git history, repository metadata, and private Actions runs for
   material that should not be public. Actions history becomes visible when
   the repository becomes public. Remove unneeded private rehearsal runs.
2. Rehearse the release workflow against the reviewed commit while the
   repository is private. Inspect the archives and checksums. Keep the new
   release tag off GitHub until the repository is public, so its first push
   triggers the public release workflow.
3. Make the repository public, check that Actions is enabled, and configure
   the tag and immutable-release protections described below. Confirm that
   repository settings allow the workflow's publication permissions.
4. Push the reviewed commit and signed tag, inspect the resulting release,
   verify its assets and attestations, and publish the crate separately with
   `cargo publish --locked`. A crates.io version cannot be replaced.
5. Update the documentation site's version and canonical documents, then
   publish it only after the GitHub release links resolve. Verify the deployed
   links and checksum instructions.

Public visibility and crate publication are separate maintainer decisions.

## Release artifacts

Every public release provides these files. `<version>` is the Cargo package
version without a leading `v`.

| File                                               | Contents and purpose                                                                  |
|----------------------------------------------------|---------------------------------------------------------------------------------------|
| `heine-<version>-x86_64-unknown-linux-musl.tar.gz` | Statically linked Linux x86-64 executable for broad Linux compatibility.              |
| `heine-<version>-x86_64-unknown-linux-gnu.tar.gz`  | Linux x86-64 executable built against Ubuntu 22.04 LTS, whose glibc baseline is 2.35. |
| `heine-<version>-x86_64-pc-windows-msvc.zip`       | Windows x86-64 executable.                                                            |
| `heine-<version>-aarch64-apple-darwin.tar.gz`      | Apple-silicon macOS executable.                                                       |
| `heine-<version>-x86_64-apple-darwin.tar.gz`       | Intel macOS executable.                                                               |
| `heine-<version>-starter-site.zip`                 | Complete runnable starter site, including its starter kit.                            |
| `SHA256SUMS`                                       | SHA-256 checksums for every archive above, but never for itself.                      |

Each executable archive contains one executable at its top level: `heine` on
Unix-like systems and `heine.exe` on Windows. The starter-site archive contains
one top-level `starter-site/` directory. It excludes generated `public/`,
development-only `scripts/`, and `.gitignore` files.

The GNU/Linux artifact uses an Ubuntu 22.04 LTS build container pinned by image
digest in the workflow. This establishes a reviewed glibc baseline rather than
depending on an evolving hosted-runner image. The workflow inspects the finished
binary's versioned dynamic symbols and records its observed GLIBC requirement in
the release notes. A build baseline is not a universal Linux compatibility
promise. Users whose systems cannot run that artifact should use the musl
archive.

The GNU build downloads a versioned `rustup-init` bootstrap binary and verifies
its SHA-256 checksum before Rustup installs the compiler pinned in
`rust-toolchain.toml`. This keeps the container and executable bootstrap inputs
reviewable; the Cargo lockfile then fixes the crate dependency resolution.

Windows and macOS artifacts are unsigned direct downloads. They remain useful,
but SmartScreen, Gatekeeper, or an organization policy can warn about or block
them. Do not instruct users to disable system-wide security controls. Checksums
and, after publication, GitHub attestations let users verify the download's
origin and contents; they do not replace platform code signing.

## Everyday changes

Before a change is considered complete, run:

The checked-in `rust-toolchain.toml` selects Heine's supported compiler and
its required components. Rustup installs it automatically when needed, or run
`rustup toolchain install` from the repository root first.

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
```

Record a user-visible change in `CHANGELOG.md` under `Unreleased` as part of
the same change. Follow its maintenance rules, including a smallest complete
migration for incompatible behavior.

## Prepare the source release

Perform these steps on the reviewed release branch, normally `main`.

1. Begin from a clean, current branch and confirm that the Forgejo and GitHub
   copies of `main` name the same commit:

   ```bash
   git switch main
   git pull --ff-only origin main
   git fetch github
   git rev-parse HEAD
   git rev-parse github/main
   ```

   The two commit IDs must match. GitHub builds the commit named by its tag, so
   a tag pushed only to Forgejo cannot create the intended GitHub release.

2. Review `Unreleased` for omissions and set the intended version in
   `Cargo.toml`. Move the unchanged entries under
   `## [<version>] - YYYY-MM-DD`, then create a new empty `Unreleased` section.
   The changelog heading and Cargo version must agree.

3. Regenerate the third-party notice from the locked dependency graph before
   packaging. Install the reviewed `cargo-about` version when it is not already
   available, then review the generated diff. A `Cargo.lock` change can add a
   new notice even when every dependency remains within the accepted license
   policy in `about.toml`.

   ```bash
   cargo install cargo-about --version 0.9.3 --locked --features cli
   cargo about generate --locked --output-file THIRD_PARTY_NOTICES.html about.hbs
   git diff --check -- THIRD_PARTY_NOTICES.html
   ```

4. Run the everyday checks, then verify the source package and a local release
   binary:

   ```bash
   cargo build --release
   cargo package
   cargo package --list
   ./target/release/heine --help
   ./target/release/heine --root starter-site
   ```

   Inspect the package listing. It must contain the intended source,
   documentation, and license material, and no local-only files. The starter
   site must build as extracted source, not only from an existing `public/`
   directory. Then confirm that Jiff cannot consult host time-zone data:

   ```bash
   features="$(cargo tree --locked -e features -i jiff)"
   printf '%s\n' "$features"
   rg 'jiff feature "tzdb-bundle-always"' <<<"$features"
   ! rg 'jiff feature "(tz-system|tzdb-bundle-platform|tzdb-zoneinfo|tzdb-concatenated)"' \
     <<<"$features"
   ```

5. Before each crates.io publication, confirm that `Cargo.toml` has the
   public canonical `repository` URL and accurate description, categories,
   keywords, license, and Rust version.

   Check keywords against the [crates.io manifest requirements](https://doc.rust-lang.org/cargo/reference/manifest.html#the-keywords-field).
   Keywords may contain at most 20 ASCII characters. The publication dry run
   described below cannot catch every registry-side rejection.

6. Commit the release preparation, push it to Forgejo, then mirror the same
   commit to GitHub. Create, verify, and push a signed annotated tag only after
   both remotes contain that commit:

   ```bash
   git push origin main
   git push github main
   git tag -s v<version> -m "Heine <version>"
   git verify-tag v<version>
   git push origin v<version>
   git push github v<version>
   ```

   The tag identifies the reviewed source revision from which artifacts are
   built. Verify it before pushing, even when ordinary commits are signed.

## Optional: private repository rehearsal

For a private repository, the release workflow is a rehearsal
tool. Start it manually for a selected signed tag or reviewed commit. It must:

1. run the same formatting, Clippy, test, package, binary-help, and starter-site
   checks as the local procedure;
2. build every archive named in [Release artifacts](#release-artifacts);
3. test each native executable on the platform that built it, and test the GNU
   binary inside the pinned Ubuntu 22.04 container;
4. inspect the GNU binary's GLIBC symbol requirements;
5. create `SHA256SUMS`, verify every listed archive with it, and upload the
   complete `dist/` directory as one private workflow artifact.

The workflow artifact is evidence for review and cross-platform testing. It is
not a public distribution channel and can expire under GitHub's artifact
retention policy. The locally generated, checksum-verified downloads on
`heine.rs` remain the pre-public distribution path.

The platform jobs retain raw executable transfer artifacts for one day. The
assembled `release-dist` artifact remains available for seven days. Deleting
the workflow run removes both kinds of artifact and its logs.

GitHub Actions can build private repositories, but standard private repositories
cannot create the public provenance attestations that users later verify with
GitHub CLI. Do not describe a private workflow artifact as attested.

Before making the repository public, run at least one complete rehearsal from
the reviewed release commit and inspect all archives. The signed release tag
must name this commit, but do not push that new tag to GitHub while the
repository is private: pushing an already existing tag after publication does
not trigger a new tag-push workflow.

```bash
sha256sum --check SHA256SUMS
tar --list --file heine-<version>-x86_64-unknown-linux-musl.tar.gz
tar --list --file heine-<version>-x86_64-unknown-linux-gnu.tar.gz
unzip -l heine-<version>-x86_64-pc-windows-msvc.zip
unzip -l heine-<version>-starter-site.zip
```

The maintainer also lists the macOS archives from the downloaded artifact:

```bash
tar --list --file heine-<version>-aarch64-apple-darwin.tar.gz
tar --list --file heine-<version>-x86_64-apple-darwin.tar.gz
```

This review catches packaging mistakes that a successful Rust compilation
cannot see.

## Workflow structure

The workflow lives at `.github/workflows/release.yml` and is committed with the
source. Forgejo stores and reviews the file but does not execute it. GitHub
Actions executes it after the same commit is mirrored to GitHub.

The workflow has two entry points:

- `workflow_dispatch` accepts a required `source_ref` input. While the
  repository is private, this is the only entry point that runs jobs. It lets a
  maintainer run the workflow definition from the default branch while checking
  out an earlier signed tag, such as `v0.8.0`.
- A pushed `v*` tag starts the public-release path only after the repository is
  public. The job condition checks the repository visibility, so pushing a tag
  while the repository remains private neither creates a release nor consumes
  platform-builder time unexpectedly. GitHub records that private tag push as
  a skipped workflow run, which the maintainer can remove with other rehearsal
  records.

The workflow uses these jobs in this order:

| Job          | Responsibility                                                                                                                                                                        | Output                                        |
|--------------|---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|-----------------------------------------------|
| `quality`    | Checks out the selected source, runs formatting, Clippy, tests, `cargo package`, release help, and a starter-site build.                                                              | Quality gate.                                 |
| `linux-musl` | Builds and runs the musl target on Linux.                                                                                                                                             | Raw `heine` executable.                       |
| `linux-gnu`  | Builds and runs the GNU target inside the pinned Ubuntu 22.04 container, then records versioned GLIBC symbols.                                                                        | Raw `heine` executable and GLIBC observation. |
| `windows`    | Builds and runs the MSVC target on a Windows runner.                                                                                                                                  | Raw `heine.exe` executable.                   |
| `macos`      | Builds and runs each macOS target on its matching macOS runner.                                                                                                                       | One raw `heine` executable per architecture.  |
| `assemble`   | Downloads only the raw executables from the preceding jobs, creates the named archives and starter-site ZIP, verifies their contents and checksums, and uploads one `dist/` artifact. | Complete release candidate.                   |
| `attest`     | Public releases only: creates provenance attestations for the assembled files.                                                                                                        | GitHub attestations.                          |
| `publish`    | Public releases only: writes release notes and creates the GitHub Release from the pre-existing tag.                                                                                  | GitHub Release assets.                        |

The quality job resolves the selected ref once and exposes its commit ID. Every
platform and assembly job checks out that immutable ID, so a manually selected
branch cannot move between jobs. The assembly job does not compile source and
does not invent artifacts. It only packages files produced by the platform
jobs. This separation ensures that every file covered by `SHA256SUMS` and later
attested came from a native target build.

The workflow uses no dependency cache. Release rehearsals are infrequent, and
avoiding a cache means retained state consists only of the job-transfer
artifacts, the assembled `dist/` artifact, and run logs.

## Remove private rehearsal data

After reviewing a private rehearsal, download its `dist/` artifact if it is
still needed locally. Then delete the completed workflow run in GitHub's
**Actions** interface. Deleting the run also deletes its artifacts and logs.
The repository owner may instead delete only the logs or only the artifact when
the run record remains useful during investigation.

Before changing the repository visibility to public, delete all private
rehearsal runs and any failed exploratory runs. The source commits, signed tags,
and committed workflow definition remain in Git history, so run rehearsals only
from source that is suitable to become public.

## Public GitHub releases

For a public repository, the signed-tag workflow runs its publication job. A
pushed `v<version>` tag must:

1. build and validate the complete artifact set from the tagged commit;
2. generate and verify `SHA256SUMS` after all archives exist;
3. create a GitHub build-provenance attestation for every distributable archive
   and for `SHA256SUMS`;
4. create the GitHub Release for that existing tag and upload exactly those
   files;
5. generate release notes that state the version, source tag, GNU/Linux glibc
   observation, unsigned Windows and macOS status, and checksum and attestation
   verification commands.

Before the first public release, enable GitHub Actions for the repository and
confirm that repository and organization policy permits the write permissions
requested below. A policy can restrict `GITHUB_TOKEN` more narrowly than the
workflow requests, causing attestation or release creation to fail.

Also configure these repository protections before pushing the first public
release tag:

1. Under **Settings → Rules → Rulesets**, create an active tag ruleset for
   `v*`. Select **Restrict updates** and **Restrict deletions**, but do not
   restrict creation. A maintainer can then create a new release tag but cannot
   accidentally move or delete it.
2. Under **Settings → General → Releases**, enable immutable releases. This
   applies only to future published GitHub Releases, not Actions rehearsal
   artifacts. GitHub then locks a published release's assets and associated tag
   and creates a release attestation that binds them to the release.

The workflow must grant only the permissions required for this job:

```yaml
permissions:
  artifact-metadata: write
  contents: write
  id-token: write
  attestations: write
```

It must pin every third-party or GitHub-maintained action by immutable commit
SHA. The release job may use the preinstalled GitHub CLI with `GH_TOKEN` set to
the workflow token. It writes the required release facts to a notes file, adds
them before GitHub's generated change summary, and creates the release from the
already pushed tag:

```bash
gh release create "v<version>" dist/* \
  --repo thomasweitzel/heine \
  --title "Heine <version>" \
  --verify-tag \
  --notes "$(cat "$RUNNER_TEMP/release-notes.md")" \
  --generate-notes
```

Uploading workflow artifacts alone does not create a GitHub Release. The
`gh release create` step creates the release entry and attaches its files under
the repository's **Releases** page.

After the job succeeds, download an archive and verify both its checksum and
provenance independently:

```bash
sha256sum --ignore-missing --check SHA256SUMS
gh attestation verify heine-<version>-x86_64-unknown-linux-musl.tar.gz \
  --repo thomasweitzel/heine \
  --signer-workflow thomasweitzel/heine/.github/workflows/release.yml \
  --source-ref refs/tags/v<version>
```

`--ignore-missing` lets a user verify the archive they downloaded without
requiring every other release file. A maintainer with the complete `dist/`
directory uses `sha256sum --check SHA256SUMS` instead.

When immutable releases are enabled, users can also verify the release-level
attestation that binds the release to its asset set:

```bash
gh release verify v<version> --repo thomasweitzel/heine
```

Confirm on the GitHub release page that all seven files are present and the
release is marked immutable. A normal release has no pre-release label; create
a discussion only when one is wanted.

## Publish to crates.io

GitHub releases do not update crates.io. Publish every crate version separately
after reviewing the GitHub release. Crates.io stores the package source, while
GitHub distributes the platform binaries and starter site.

For the first publication, sign in to [crates.io](https://crates.io/) with the
publishing GitHub account, verify the account email, and check that the `heine`
name is available to that account. Create an API token with the permission to
publish a new crate. Later publications require permission to publish updates.
Run `cargo login` and enter the token at its prompt. Keep the token out of the
repository, shell history, logs, and messages. See the [Cargo publishing
guide](https://doc.rust-lang.org/cargo/reference/publishing.html) for account
and authentication details.

Publish from the exact tagged commit. If the working checkout has moved on,
create a separate worktree, substituting the release version:

```bash
git worktree add --detach /tmp/heine-release-<version> v<version>
cd /tmp/heine-release-<version>
git status --short --branch
git rev-parse HEAD
git rev-parse 'v<version>^{commit}'
```

The worktree must be clean and the commit IDs must match. Inspect the package
and perform a dry run before uploading:

```bash
cargo package --locked --list
cargo publish --locked --dry-run
cargo publish --locked
```

The final command uploads a permanent version. A dry run builds the package but
cannot test all checks that crates.io applies to an actual upload. If Cargo
times out while waiting for the index after uploading, check crates.io before
trying again; the upload may have succeeded. Once published, check the version
on the [Heine crate page](https://crates.io/crates/heine) and with:

```bash
cargo search heine --limit 3
cargo info heine@<version>
```

If crates.io rejects an upload, read the registry error and confirm whether
the version exists before retrying. Correct the source and repeat the package
checks. Never move a published GitHub tag. Prefer a new release version when
the correction changes release source. If an unpublished crate version must be
published from a different commit for a metadata-only correction, explicitly
record and review the difference, publish from a clean commit, and carry the
correction into `main`. The GitHub tag and that crate then identify different
commits, so do not claim otherwise.

## Update the documentation site

Update the version and canonical documents for `heine.rs`. Its download page
links directly to the GitHub Release assets. Publish the site after the assets
exist, then check the rendered download page and every linked file. Confirm
the crates.io link and installation instructions after the crate is available.

## Recover from a failed, canceled, or timed-out run

A timed-out job is canceled and marked failed. Jobs that depend on it do not
run. A platform-build timeout therefore prevents assembly, attestation, and
publication; an assembly timeout prevents attestation and publication; and an
attestation timeout prevents publication.

When a private rehearsal fails, inspect or download any completed artifacts if
they are useful, then delete the workflow run when the investigation ends. No
GitHub Release exists in this stage.

For a public tag run, first inspect the GitHub Release state:

1. If no release exists, a transient runner or network failure may be rerun
   from the same workflow run. The rerun uses the same committed workflow and
   tagged source. If the tagged workflow itself cannot publish, obtain the
   assembled artifact from that tag's successful build and attestation jobs.
   Verify its checksums and provenance, then create a release for the existing
   tag with those exact files. Include the release facts and verification
   instructions described above; do not substitute a private rehearsal build.
   In GitHub Actions, open the successful tag run and download `release-dist`
   before its seven-day retention expires. In **Releases**, create a draft for
   the existing tag, attach all seven individual files, and review their names,
   checksums, notes, and provenance before publishing. Immutable-release
   protection applies when the draft is published.
2. If a draft exists, inspect it. The GitHub CLI creates a draft before it
   uploads assets, so a timeout during publication can leave a partial draft.
   Delete that draft before rerunning the publication job. Immutable-release
   protection does not apply until a draft is published.
3. If a release was published, do not modify its assets or tag. Verify its
   assets and release attestation, then publish a corrected new version if one
   is necessary.

If resolving the failure requires a source or workflow change, prepare a new
release version and signed tag. A protected or immutable release tag must never
move to make a failed release appear to have succeeded.

## Workflow boundaries

The workflow reproduces this procedure. It does not replace local review or
expand Heine into a deployment system.

- Build source always comes from the triggering tag or manually selected GitHub
  ref, never from an untracked workspace.
- The action does not publish to crates.io.
- The action does not deploy `heine.rs`.
- The action never overwrites an existing GitHub Release or tag.
- The workflow does not add an advisory, dependency-source, duplicate-version,
  or license-policy check until Heine has an explicit policy for it.
