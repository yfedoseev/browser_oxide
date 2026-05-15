# DataDome Encryption — Authoritative Reference (2026-05-15)

Source: `glizzykingdreko/datadome-encryption` `src/encryption.js` (clean-room
Node reimplementation), fetched + analysed verbatim 2026-05-15. This is
the W3.8 solver's payload encoder for the `rt:'i'` interstitial path
(etsy/tripadvisor/wsj/reuters). Porting this to Rust is a transcription
task — the algorithm below is complete; no further RE needed.

## Constants

| | captcha | interstitial |
|---|---|---|
| `_mainPrngConstant` | 9959949970 | 9959949970 |
| `_hashXorConstant` | -1748112727 | **-883841716** |
| `_cidPrngConstant` | 1809053797 | 1809053797 |

All integer ops are JS 32-bit semantics: `| 0`, `<<`, `>>` (arithmetic/
signed), `>>>` where noted, wrap on overflow. Port with `i32`/`u32`
wrapping_*; `>>` = arithmetic (i32), `>>>` = logical (u32).

## Primitives (deterministic — port + unit-test first)

```
_customHash(str):
  if str empty -> return 1789537805
  hash = 0 (i32)
  for ch in str.charCodeAt:  hash = ((hash << 5) - hash + ch) | 0   // ×31 djb2-variant
  return hash != 0 ? hash : 1789537805

_mixInt(v):           // xorshift32, JS signed shifts
  v ^= v << 13
  v ^= v >> 17        // arithmetic (signed) >>
  v ^= v << 5
  return v | 0

_encode6Bits(v):      // custom base64 codepoint map
  if v > 37: 59 + v
  elif v > 11: 53 + v
  elif v > 1: 46 + v
  else: 50 * v + 45
```

## PRNG (stateful, returns a closure)

```
_createPrng(seed, salt) -> fn(flag):
  state = seed; round = -1; saltState = salt
  useAlt = self._useAlt; self._useAlt = false   // only FIRST prng gets useAlt=true
  cache = null
  fn(flag):
    if cache != null: result = cache; cache = null
    else:
      if ++round > 2: state = _mixInt(state); round = 0
      result = state >> (16 - 8*round)           // signed >>
      if useAlt: result ^= (--saltState)
      result &= 255
      if flag: cache = result
    return result
```

## Seed / salt derivation

```
_resetEncryptionState():
  _useAlt = true
  _prngSeed = _mainPrngConstant ^ _customHash(hash) ^ _hashXorConstant
  _salt = externalSalt if provided
          else _mixInt(_mixInt((Date.now()>>3) ^ 11027890091) * _mainPrngConstant)
  _prng = _createPrng(_prngSeed, _salt)         // consumes the one useAlt=true
  cidPrngSeed = _cidPrngConstant ^ _customHash(cid)
```

For deterministic tests/parity: pass an explicit `externalSalt` (the
interstitial path supplies the salt; for byte-parity vs the reference,
run their Node harness with a pinned salt and capture the output).

## Buffer construction (per signal)

```
_utf8Xor(str, prng): UTF-8 encode str (standard, incl. surrogate pairs),
                      then byte[j] ^= prng()  for all bytes

_addSignal(key, value):   // value must be number|string|boolean (or falsy)
  skip if key=='xt1' or key empty or value invalid
  keyStr   = JSON.stringify(key)
  valueStr = JSON.stringify(value)
  startByte = _prng() ^ (buffer.empty ? 123 : 44)   // '{' or ','
  buffer.push(startByte)
  buffer.extend(_utf8Xor(keyStr, _prng))
  buffer.push(58 ^ _prng())                          // ':'
  buffer.extend(_utf8Xor(valueStr, _prng))
```

## Final assembly

```
_buildPayload(cid):
  cidPrng = _createPrng(_cidPrngConstant ^ _customHash(cid), _salt)  // useAlt=false here
  out = [ buffer[i] ^ cidPrng()  for i ]
  out.push(125 ^ _prng(true) ^ cidPrng())            // '}' terminator, flag=true
  return _encodePayload(out, _salt, _encode6Bits)

_encodePayload(byteArr, salt, enc):
  n = salt; output = []
  for groups of 3 bytes (i steps by 3):
    chunk = ((255 & --n ^ b[i])   << 16)
          | ((255 & --n ^ b[i+1]) << 8)
          | ( 255 & --n ^ b[i+2])
    push enc((chunk>>18)&63), enc((chunk>>12)&63),
         enc((chunk>>6)&63),  enc(chunk&63)   as chars
  mod = byteArr.length % 3
  if mod: drop last (3 - mod) chars
  return output joined
```

## Validation vectors (from the repo's tests/)

- `cid = "k6~sz7a9PBeHLjcxOOWjR162xQq2Uxsx6wLzxeGlO7~6k3JVwDkwAaQ04wdFEMm2Jt2s0y61mLfJdhWuqtqeJzFMuo7Lf8P5btYX0K4EeoLRcNAtNW04rGhTE3nKpMxi"`
- `hash = "14D062F60A4BDE8CE8647DFC720349"`
- signals: `tests/original.json` (array of [key,value] pairs)
- expected: `tests/excepted.txt` (compare **ignoring the last char** —
  it is salt/timestamp-dependent; the rest is deterministic given the
  same salt sequence the reference used). For a hard byte-parity test,
  pin `externalSalt` in BOTH our port and a one-off Node run of the
  reference and capture that output as the fixture.

## Port plan (next loop iterations)

1. ✅ This doc (reference captured — no more RE).
2. Port primitives `_customHash`/`_mixInt`/`_encode6Bits` + unit tests
   (hand-computed vectors) — small, exact.
3. Port PRNG closure + seed/salt derivation + unit test the prng byte
   stream for a pinned (seed,salt).
4. Port buffer construction + `_buildPayload` + `_encodePayload`.
5. Byte-parity test: pinned-salt fixture captured from a one-off Node
   run of the reference against tests/original.json.
6. Wire into `datadome_handler`: on a detected `rt:'i'` interstitial,
   build the §3 signal map, encrypt, POST to
   `<host>/interstitial/` , capture `Set-Cookie: datadome=`, re-issue.
   (The §3 signal map + daily 6-char wire-key dictionary is the
   remaining sub-problem — `03_DATADOME.md` §3/§4.4.)

Steps 2-5 are pure deterministic Rust + tests (no network, no V8) —
ideal loop-iteration units. Step 6 is the integration + the signal-map
sub-problem. 4-site union impact (etsy/tripadvisor/wsj/reuters); yelp
excluded (`t:'bv'` IP hard-ban).
