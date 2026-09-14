# Contributing to Heine

Thanks for your interest in Heine. This guide explains the project's scope,
the contributions that fit it, and when an issue should come before a pull
request.

## What Heine is, and who it's for

Heine is built primarily for its author's own needs: checked builds, Fluent
localization, and a focused set of features, including margin footnotes,
Series navigation, REUSE-compatible asset licensing, syntax highlighting, and
color-scheme Markdown pictures. These address concrete problems with existing
static site generators.

It is shared publicly because other people may find those solutions useful,
not because it is a community-directed project. Feature direction is set by
the maintainer. Stating that boundary up front helps contributors decide where
their effort is likely to fit.

## Non-goals

The following broader directions are not planned. Issues and pull requests
proposing them will generally be closed without extensive discussion:

- A general-purpose plugin or shortcode system. Renderer adapters are the
  only planned extension boundary. They are not implemented yet and will not
  grow into a broader plugin architecture.
- Multi-core / parallel / async builds. Heine's single-threaded,
  synchronous build is a deliberate choice that keeps its correctness
  guarantees simple to reason about and verify. This is not considered
  a limitation to "fix."
- Support for extreme-scale sites (six-figure page counts and beyond).
  Heine is scoped for small-to-mid sites; if you need that scale, Hugo
  is a better fit today.
- Becoming a general frontend toolchain: CSS/JS bundling, image
  transforms, deployment, or asset pipelines. Use dedicated tools for
  these and let Heine consume their output.
- A general theming framework or unrestricted kit composition. The planned kit
  model selects one checked presentation layer rather than treating kits as
  plugins.

If any of the above is a hard requirement, fork Heine rather than waiting for
this project to change direction. Its license permits that.

## What's genuinely welcome

- **Bug reports.** If Heine builds something incorrectly, silently
  overwrites output, mishandles a locale, or produces broken checking
  behavior, that's a real bug regardless of how small. Please report
  it with a minimal reproduction if you can.
- **Documentation fixes.** Typos, unclear wording, missing examples in
  the guides or reference. Small PRs here are easy to review and
  genuinely appreciated.
- **Small, self-contained bug-fix PRs.** If the fix is obviously correct
  and scoped to the bug, go ahead and open the PR directly.

## Feature requests and large changes

- **Feature requests.** These will be read and genuinely considered, but
  not every one will get a reply, and most will not be implemented. The
  [roadmap](https://heine.rs/docs/roadmap/) distinguishes planned work from
  later or undecided work. A planned item shows current direction, not a fixed
  timeline or a commitment to a particular interface. If something is not
  listed there, assume it is not currently planned.
- **Large feature PRs.** Please open an issue to discuss *before*
  writing a substantial PR. This avoids investing time in a change that does
  not fit Heine's scope and would be declined afterward.

## How issues get closed

If an issue or PR is closed as out of scope, it's a statement about fit
with this project's specific, narrow goals, not a judgment on the idea
or the effort behind it. Forking is a legitimate and expected outcome
for anyone who wants Heine to go a different direction than the
maintainer does.

## Before opening a pull request

Run the full local quality check:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
```

Keep a pull request focused. A user-visible behavior change needs focused
tests and relevant documentation. Update `CHANGELOG.md` and the starter site
when the change naturally affects them.

## License

Heine is licensed under EUPL 1.2 or later. By contributing, you agree
your contribution is provided under the same license.
