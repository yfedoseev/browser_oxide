#!/bin/bash
# Extracts (or validates) release notes for a given version from CHANGELOG.md.
#
# Usage:
#   extract-release-notes.sh <version>           # write release-title.txt + release-notes.md
#   extract-release-notes.sh --check <version>   # validate only, write nothing, exit non-zero on problems
#
# A well-formed CHANGELOG section looks like:
#
#   ## [0.1.0] - 2026-06-13
#
#   > One-line (or multi-line) subtitle describing the release.
#   > Continuation lines are concatenated into a single subtitle.
#
#   ### Added
#   - ...
#
# The script is STRICT: it fails loudly — instead of silently producing a stale
# or bare title — when, for the requested version:
#   * the `## [VERSION]` section is missing entirely, or
#   * no `> ...` subtitle blockquote appears directly under the header, or
#   * the section has no `### ` heading (i.e. it's an empty stub).
#
# The subtitle scan is BOUNDED to the requested version's own section AND
# ANCHORED to the blockquote that immediately follows the version header (only
# blank lines may sit between), so it can never scrape an older version's
# blockquote. A multi-line blockquote is concatenated rather than truncated.
# Body blockquotes later in the section are preserved in the release notes —
# only the leading subtitle block is stripped from the body.

set -euo pipefail

CHECK_ONLY=0
if [ "${1:-}" = "--check" ]; then
  CHECK_ONLY=1
  shift
fi

VERSION="${1:?usage: extract-release-notes.sh [--check] <version>}"
CHANGELOG="CHANGELOG.md"

if [ ! -f "$CHANGELOG" ]; then
  echo "::error::$CHANGELOG not found" >&2
  exit 1
fi

# 1. The version section must exist. Match the bracketed token literally
#    (string compare, not regex) so dots in the version aren't wildcards.
if ! awk -v ver="$VERSION" '
  /^## \[/ { s=$0; sub(/^## \[/,"",s); sub(/\].*/,"",s); if (s==ver) { found=1; exit } }
  END      { exit(found ? 0 : 1) }
' "$CHANGELOG"; then
  echo "::error file=CHANGELOG.md::No '## [$VERSION]' section found in CHANGELOG.md. Add the release section (with a '> subtitle' and '### ' notes) before tagging." >&2
  exit 1
fi

# 2. Extract the subtitle: the contiguous run of '>' lines that IMMEDIATELY
#    follows the version header (only blank lines may precede it). Bounded by
#    the next '## [' header (or EOF). If the first non-blank line in the
#    section is not a '>' blockquote, the subtitle is treated as MISSING.
SUBTITLE=$(awk -v ver="$VERSION" '
  function hdrver(line,   s) { s=line; sub(/^## \[/,"",s); sub(/\].*/,"",s); return s }
  /^## \[/ {
    if (hdrver($0) == ver) { in_section=1; pre=1; next }
    if (in_section) exit            # reached the next version → stop
    next
  }
  in_section {
    if (pre && $0 ~ /^[ \t]*$/) next   # blank lines between header and subtitle
    if (pre) {                         # first non-blank line in the section
      if ($0 !~ /^>/) exit             # not a blockquote → no anchored subtitle
      pre=0
    }
    if ($0 ~ /^>/) {
      l=$0; sub(/^>[ \t]?/,"",l)
      st = (st=="" ? l : st " " l)
    } else {
      exit                             # end of the subtitle blockquote
    }
  }
  END { if (st != "") print st }
' "$CHANGELOG")

if [ -z "$SUBTITLE" ]; then
  echo "::error file=CHANGELOG.md::No '> subtitle' blockquote found under '## [$VERSION]' in CHANGELOG.md. Add a one-line (or multi-line) '> ...' subtitle directly below the version header before tagging." >&2
  exit 1
fi

# 3. The section must contain at least one '### ' heading (real notes, not a
#    bare stub). Bounded to this version's section.
if ! awk -v ver="$VERSION" '
  function hdrver(line,   s) { s=line; sub(/^## \[/,"",s); sub(/\].*/,"",s); return s }
  /^## \[/ { if (hdrver($0)==ver){in_section=1;next} if(in_section) exit; next }
  in_section && /^### / { found=1; exit }
  END { exit(found ? 0 : 1) }
' "$CHANGELOG"; then
  echo "::error file=CHANGELOG.md::Section '## [$VERSION]' has no '### ' heading — it looks like an empty stub. Add the real release notes before tagging." >&2
  exit 1
fi

TITLE="v${VERSION} | ${SUBTITLE}"

if [ "$CHECK_ONLY" -eq 1 ]; then
  echo "CHANGELOG OK for v${VERSION}: ${TITLE}"
  exit 0
fi

echo "$TITLE" > release-title.txt

# Extract body: everything between this version's header and the next '## [',
# minus ONLY the leading subtitle block (the blank lines + the contiguous '>'
# run immediately under the header). Body blockquotes later in the section
# (scope notes, callouts) are preserved verbatim.
awk -v ver="$VERSION" '
  function hdrver(line,   s) { s=line; sub(/^## \[/,"",s); sub(/\].*/,"",s); return s }
  /^## \[/ { if (hdrver($0)==ver){in_section=1;phase="lead";next} if(in_section) exit; next }
  in_section {
    if (phase=="lead") {
      if ($0 ~ /^[ \t]*$/) next        # blank lines above the subtitle
      if ($0 ~ /^>/) { phase="sub"; next }
      phase="body"                     # defensive: no anchored subtitle
    }
    if (phase=="sub") {
      if ($0 ~ /^>/) next              # subtitle blockquote line — strip
      phase="body"                     # subtitle ended; print this line onward
    }
    print
  }
' "$CHANGELOG" \
  | sed '1{/^$/d}' > changelog-section.md

if [ ! -s changelog-section.md ]; then
  echo "::error file=CHANGELOG.md::No changelog body content found for version ${VERSION}" >&2
  rm -f changelog-section.md
  exit 1
fi

# Build release body = changelog section + installation footer
cat changelog-section.md > release-notes.md
cat >> release-notes.md << 'FOOTER'

---

### Installation

**Rust (crates.io)**
```bash
cargo add browser_oxide
```

**Python (PyPI)**
```bash
pip install browser-oxide
```

**MCP Server (for AI assistants)**
```bash
cargo install browser_oxide_mcp
```

Or download a pre-built `browser-oxide-mcp` binary for Linux, macOS, or Windows
from the assets below.

### Platform Support (MCP binary)
| Platform | Architecture | Archive |
|----------|-------------|---------|
| Linux | x86_64 (glibc) | `browser-oxide-mcp-x86_64-unknown-linux-gnu.tar.gz` |
| macOS | ARM64 (Apple Silicon) | `browser-oxide-mcp-aarch64-apple-darwin.tar.gz` |
| macOS | x86_64 (Intel) | `browser-oxide-mcp-x86_64-apple-darwin.tar.gz` |
| Windows | x86_64 | `browser-oxide-mcp-x86_64-pc-windows-msvc.tar.gz` |

### Changelog
See [CHANGELOG.md](https://github.com/yfedoseev/browser_oxide/blob/main/CHANGELOG.md) for full details.
FOOTER

# Cleanup
rm -f changelog-section.md

echo "Generated release-title.txt and release-notes.md for v${VERSION}"
echo "Title: ${TITLE}"
