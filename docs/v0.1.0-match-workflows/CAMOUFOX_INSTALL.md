# Camoufox install — v135 (stable) vs v150 (preview), and the launcher↔browser pin

> Why this matters: the full-gate competitor comparison failed because the
> cached camoufox browser binary didn't match the installed python launcher.
> This documents the **correct** install for each version and the gotcha.

## TL;DR

| Version | On PyPI pip channel? | Install command | Status here |
|---|---|---|---|
| **v135.0.1-beta.24** | ✅ yes (latest stable) | `pip install -U camoufox[geoip] playwright && python -m camoufox fetch` | **WORKS** (smoke-tested) |
| **v150.0.2-beta.25** | ❌ **no** — github *preview* only | not installable via the stable launcher (see §3) | crashes the 0.4.11 launcher |

## 1. The launcher↔browser version pin (the gotcha)

The `camoufox` **python package** (the launcher) is hard-pinned to the **browser
binary release** it knows how to drive (asset naming, driver protocol). They
must match:

- Latest PyPI launcher = **`camoufox==0.4.11`** → drives **browser
  v135.0.1-beta.24**.
- `python -m camoufox fetch` has **no `--version` flag** — it downloads the
  first github release whose Linux/arch asset the *current launcher* recognizes.
  For 0.4.11 that resolves to **v135.0.1-beta.24** (it skips v150 because the
  v150 asset/protocol changed and 0.4.11 doesn't recognize it).
- Putting a **v150 binary** under `~/.cache/camoufox` with the **0.4.11 launcher**
  → every `Browser.new_page()` dies with `Connection closed while reading from
  the driver` (launcher/binary protocol mismatch). This is what broke our gate.

**Verify the pair matches:**
```bash
/tmp/bo-venv/bin/pip show camoufox | grep Version        # launcher
cat ~/.cache/camoufox/version.json                        # browser binary
# launcher 0.4.11  <->  browser "135.0.1" / "beta.24"  = matched, OK
```

## 2. Correct install of camoufox v135 (stable, supported)

```bash
python3 -m venv /tmp/bo-venv
/tmp/bo-venv/bin/pip install -U "camoufox[geoip]" playwright
/tmp/bo-venv/bin/python -m camoufox fetch          # -> v135.0.1-beta.24
```
Smoke-test (must print a real length):
```bash
/tmp/bo-venv/bin/python - <<'PY'
import asyncio
from camoufox.async_api import AsyncCamoufox
async def m():
    async with AsyncCamoufox(headless=True) as b:
        p = await b.new_page(); await p.goto("https://example.com/")
        print("OK", len(await p.content()))
asyncio.run(m())
PY
```
Run the corpus per-site isolated (driver is unstable in a sustained loop —
relaunch per site): `benchmarks/run_camoufox_isolated.py`.

## 3. camoufox v150 — why it isn't pip-installable (yet) and the options

Per the project (camoufox.com / GitHub issue #613): **v150.0.2 is a *preview*
github release**, marked production-ready *"once extended testing is completed,
then made available on the pip packages"* (Windows build still pending). It is
**not on the PyPI channel**, so the stable launcher cannot fetch or drive it.

Options to actually run v150 (none are "stable/supported"):
1. **Wait** for v150 to land on the pip channel (then `pip install -U camoufox`
   will pull a launcher that drives it).
2. **Git/dev launcher**: install camoufox's python lib from `main`
   (`pip install "camoufox[geoip] @ git+https://github.com/daijro/camoufox.git"`)
   IF/when main supports the v150 asset format, then `camoufox fetch`. Unverified
   here — treat as experimental.
3. **Manual**: download the `v150.0.2-beta.25` Linux asset from
   `github.com/daijro/camoufox/releases`, place under `~/.cache/camoufox`, AND
   pair it with a launcher/playwright-firefox driver built for v150. Fragile;
   the version.json must read 150.x and the launcher must match.

## 4. What this means for the gate / benchmarks

- **v135** is the reliably-installable camoufox today → use it for same-run
  competitor data (`run_camoufox_isolated.py`, per-site relaunch for stability).
- **v150** comparisons should use the **documented 2026-05-27 baseline (~112-113)**
  until v150 reaches the pip channel; do not place a v150 binary under a v135
  launcher (it will crash).
- Always assert launcher↔binary match (§1) at the top of any competitor run.

Sources: https://github.com/daijro/camoufox · https://camoufox.com/python/installation/ ·
https://pypi.org/project/camoufox/ · https://github.com/daijro/camoufox/releases ·
https://github.com/daijro/camoufox/issues/613
