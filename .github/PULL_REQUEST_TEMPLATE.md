<!-- Thanks for the PR. Please fill out below. The title should follow conventional commits, e.g. `feat(stealth): …`, `fix(net): …`, `docs: …`. -->

## What changed

<!-- One paragraph. What problem does this solve, and how? -->

## Type

- [ ] Bug fix
- [ ] Feature
- [ ] Refactor (no behavior change)
- [ ] Performance
- [ ] Docs / examples
- [ ] CI / tooling
- [ ] Breaking change (see "Breaking changes" below)

## Related issues

<!-- "Fixes #123", "Refs #456", etc. -->

## How tested

<!-- What did you run? `cargo test -p <crate>` is the bare minimum; for engine-level work include a holistic sweep result if relevant. -->

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace -- -D warnings`
- [ ] `cargo test --workspace -- --test-threads=1` (V8 single-thread)
- [ ] `cargo doc --no-deps --workspace` (no rustdoc warnings)

## Breaking changes

<!-- If yes, describe the migration path. Otherwise delete this section. -->

## Scope

<!-- Confirm the change fits the project's scope per SCOPE.md. -->

- [ ] Use case is consistent with SCOPE.md (archival, accessibility, AI agents, security research, CTF).
- [ ] If this touches the stealth surface, the change is engineering-level (parity with a documented Chrome / Firefox behavior), not site-specific exploit code.

## Checklist

- [ ] Self-reviewed
- [ ] No new warnings
- [ ] No new unsafe blocks without a `// SAFETY:` comment
- [ ] Public API additions have rustdoc
- [ ] Conventional-commit title
