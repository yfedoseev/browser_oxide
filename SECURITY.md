# Security Policy

## Supported versions

While the project is pre-1.0 (`0.1.x`), only the latest commit on `main`
is supported. Once the project tags its first stable release, this
section will list supported version lines.

## Reporting a vulnerability

**Please do not file public GitHub issues for security vulnerabilities.**

Use one of these channels:

1. **GitHub Security Advisories (preferred):** open a private advisory
   at <https://github.com/yfedoseev/browser_oxide/security/advisories/new>.
2. **Email:** the maintainer's email is in
   [CONTRIBUTING.md](CONTRIBUTING.md) and on the GitHub profile.

### What to include

- The type of issue (memory unsafety, sandbox escape, parsing-DoS, etc.).
- The file path(s) and the function or line numbers involved.
- A minimal reproduction — Rust snippet, HTML/CSS/JS fragment, or
  `cargo test …` invocation.
- Whether the issue is reachable in the default configuration or only
  in a specific feature flag / build profile.
- Any temporary mitigation you found while testing.

### What we will do

- Acknowledge the report within **2 business days**.
- Triage and assign severity within **7 days**.
- For high/critical issues we aim to ship a patch release within 30
  days; for lower-severity issues we batch into the next minor.
- Coordinate disclosure with you. We default to a 90-day window from
  initial report unless circumstances warrant otherwise.

### Scope

In scope:
- Memory safety issues (`unsafe` blocks misuse, FFI boundary bugs).
- Sandbox / isolation gaps (V8 isolate escape, iframe origin bleed,
  worker scope violations).
- TLS / HTTP protocol implementation bugs.
- Denial-of-service via malformed HTML/CSS/JS input.
- Stealth-profile bugs that expose an unintended fingerprint signal
  (i.e., the engine outputs something a real browser wouldn't).

Out of scope:
- Anti-bot vendor detection failures *per se*. Many sites use
  multi-layered detection (IP reputation, behavioural, paid solver
  farms) that no fingerprint-only engine can fully address. If you
  believe a specific signal we emit *differs from real Chrome/Firefox
  in a way we missed*, that's in scope (file under "Stealth-profile
  bugs"). "Site X blocks us" alone is not a security issue.
- Vulnerabilities in upstream dependencies — please report those to
  the dependency authors. We will update once an upstream fix lands.

## Recognition

We'll credit the reporter in the changelog and release notes unless you
prefer to remain anonymous. The project does not currently offer a bug
bounty.
