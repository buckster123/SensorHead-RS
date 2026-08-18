# SensorHead-RS — Agent & Developer Guide

> Physical senses for an ApexOS Pi — thermal, air, cameras — with Rust owning
> everything that is not a named vendor wall.
> One Cargo workspace: core lib, then thin MCP / CLI / HTTP faces as they earn a crate.
> ApexOS-RS is the first consumer (HTTP on `:8080`). Standalone is a first-class goal.

Bootstrapped 2026-08-18. House conventions come from `~/Projects/Launchpad-RS/`
— load a doc from there when you need the detail behind a rule below.

**Read `docs/CHARTER.md` before any non-trivial change — its decisions log (D1–Dn) is
binding.** Amend it with a dated entry when a decision changes, never silently. Where the
charter and this file disagree, the charter wins.

Siblings: `../ApexOS-RS` (consumes `:8080` via `apex-sensor-bridge` + gateway; do not
assimilate this repo into that workspace). Reference/read-only source:
`py-source/` (`buckster123/SensorHead` — **do NOT modify**; see `docs/upstream.md`).

---

## What this is

The SensorHead option on an ApexOS Pi 5 (live on apex1): BME688 air, MLX90640 thermal,
IMX500 AI camera, IMX708 NoIR. The original is Python. This repo ports as far as Rust
can honestly go, and **links** only where a vendor stack forces C/C++ or Python.

```
crates/
  sensorhead/         # core lib — types, parsers, ironbow, degrade rules
  sensorhead-api/     # axum HTTP face — drop-in :8080; walls via SENSORHEAD_UPSTREAM
docs/design.md        # THE contract — HTTP + MCP, pinned from the Python original
docs/upstream.md      # py-source checkout + the two FFI walls
docs/CHARTER.md       # binding decisions
BACKLOG.md            # slice ledger
py-source/            # local clone of buckster123/SensorHead (gitignored)
```

---

## Locked decisions

The load-bearing summary; **`docs/CHARTER.md` D1–Dn is the binding long form.** House defaults
are pre-filled — delete what doesn't apply, add what's yours.
**Locked means locked — do not re-litigate these mid-session; amend deliberately, with a date.**

- **Language**: Rust — one Cargo workspace, every binary in it
- **Pure Rust, no C linking** where a Rust crate exists (see `Launchpad-RS/docs/stack.md`)
- **Named FFI walls** (charter D2): Bosch **BSEC2** (C, via the pi3g `bme68x` bindings) and
  Raspberry Pi **libcamera / Picamera2** (C++, IMX500 on-chip AI). Everything else is a
  candidate for native Rust.
- **MCP**: hand-rolled newline-delimited JSON-RPC over stdio, protocol `2024-11-05`, no SDK
- **HTTP**: `reqwest` (rustls) out, `axum` in; `clap` for CLI; `serde` everywhere
- **Wire**: keep the Python dashboard's `:8080` JSON contract so ApexOS-RS does not need a
  coordinated change (charter D4)
- **CI from commit 0**: fmt `--check` + clippy `-D warnings` + test + build
- **rustfmt-clean baseline from commit 0** — so `cargo fmt --all` is always safe here
- **Nano-first**: **cut as a hardware target** — the head is Pi 5 class. The daemon must
  still start and honestly degrade when sensors are absent
- **Storage**: no SQLite in v1. BSEC calibration state stays a JSON blob owned by the
  BSEC sidecar (`bsec_state.json`), same as the Python original

---

## The playbook (the house method — read once, then live it)

Full rationale: `~/Projects/Launchpad-RS/docs/house-doctrine.md`. The nine, condensed:

1. **Contract first.** Pin the wire/API/format in `docs/design.md` before code. Code follows
   docs; a PR updates both. **Docs travel with code.**
2. **Slices, not marathons.** One branch = one reviewable slice off freshly-fetched
   `origin/main`. Never open a PR whose base is another branch.
3. **Honesty invariants.** Never a fake success. Degrades are *stated* ("no key configured"
   beats a timeout). Failures carry the real reason. Check the response body, not just the
   HTTP status. Never silently clamp what you can honestly reject.
4. **Pure-fn test discipline.** Pure functions (parsers, builders, state mappers, ranking,
   formatting) are the unit-test surface; handlers are thin I/O glue. Upstream parsers get a
   fixture test built from real captured JSON. Effectful e2e tests skip *loudly*.
5. **Field truth beats green CI.** A slice is done when it runs on a live node — screenshots,
   real jobs, real output — not when tests pass. The ledger row gets its ✅ only then.
6. **Secrets hygiene.** Never print a key or token (lengths and heads only). Never write one
   into a repo, a transcript, a doc, or a non-0600 file. **No credentials in CLAUDE.md** —
   these files get committed, and repos go public.
7. **Cerebro is the thread.** `session_recall` at start, `session_save` at milestones and end.
8. **Spend is gated.** Paid operations (API credits, GPU rental, image/music generation) never
   auto-fire from a default flow. Live-fire runs are explicit, counted, and André's call.
9. **Cost the failure, not the happy path.** A paid job that outlives its poll window is
   *pending*, not failed — leave it recoverable (resumable ids), never orphan spend.

---

## Git discipline

- **Never commit to `main`.** Feature branch off freshly-fetched `origin/main`: `feat/…`,
  `fix/…`, `chore/…`, `docs/…`. One branch = one slice.
- **Ship via PR** (`gh pr create`). **This repo: FORGE may merge** (charter D8,
  2026-08-18) after `cargo test --workspace` and clippy `-D warnings` are green.
  Still one branch per slice; still no commit directly to `main`; still no
  force-push.
- **Commit format:** imperative, lowercase. End with the `Co-Authored-By` trailer.
- **Never amend a pushed commit. Never force-push.**
- **Push after every commit.** Local git is the floor of resilience: if Cerebro is
  unavailable, the repo + its docs must be enough to reconstruct full project context.

---

## Cerebro session protocol (mandatory)

All Cerebro MCP calls use agent FORGE (`agent_id="FORGE"`) — memories stay isolated per project.
Full tool menu + grading discipline: `~/Projects/Launchpad-RS/docs/cerebro-protocol.md`.

**Session START** — before touching any code:
```
session_recall(query="SensorHead-RS build status step progress", agent_id="FORGE")
```

**Session END** (and at milestones on long sessions):
```
session_save(session_summary="what was built, what broke, what was learned",
             key_discoveries=[...], unfinished_business=[...],
             agent_id="FORGE", priority="HIGH")
```
Then as needed: `store_procedure` · `record_procedure_outcome` (**grade every procedure you
exercised** — ungraded ones are invisible to the dream engine) · `store_intention` (parked
ideas, salience 0.8–0.95) · `episode_*` (multi-step sequences).

**The vaults:** CLAUDE.md = lean core + pointers · `docs/gotchas.md` = invariants ·
`docs/*.md` = per-topic detail · Cerebro = session memory, survives compaction · git = code truth.

---

## Dev commands

```bash
cargo test --workspace
cargo fmt --all && cargo clippy --workspace -- -D warnings   # clippy-zero policy
cargo build --release --workspace
# Drop-in face in front of the live Python dashboard (laptop → apex1):
cargo run -p sensorhead-api -- --bind 127.0.0.1:18080 --upstream http://192.168.0.158:8080
# Refresh the Python original (gitignored nested checkout):
git -C py-source pull --ff-only
```

Pi + systemd + env-file pattern: `~/Projects/Launchpad-RS/docs/deploy.md`.
The live Python unit on apex1 is the field truth until this repo replaces it.

---

## Gotchas

Project invariants live in **`docs/gotchas.md`** — grep it for your subsystem **before**
modifying it. Most entries were written after something broke on a live node; each ends with
an explicit "don't do X". **A new gotcha goes THERE, not here.** Cross-project version drift
(axum/comrak/gix/tantivy/slint/wgpu/…) is in `~/Projects/Launchpad-RS/docs/sharp-edges.md`.

Two that bite every project in this garden:

- **MCP stdout is sacred.** All `tracing`/log output goes to **stderr**. A stray `println!`
  corrupts the JSON-RPC stream.
- **Read the pinned crate's docs for the exact version** — not memory of an older API.
  Version drift gets recorded in a dated changelog line, never fought silently.

---

## Docs

Load only the relevant doc when entering a subsystem — do not load all of them.

| File | Load when working on |
|------|----------------------|
| `docs/CHARTER.md` | **Binding decisions D1–Dn, phases, scope fence — read before non-trivial work** |
| `docs/design.md` | **The contract** — wire format, API, invariants |
| `docs/upstream.md` | py-source checkout, BSEC2 + libcamera walls, what can be native Rust |
| `docs/gotchas.md` | **Any subsystem change — grep it first** |
| `BACKLOG.md` | Outstanding work — slice ledger + parked items |
| `crates/sensorhead-api` | HTTP drop-in — load when changing routes or the upstream proxy |
| `docs/deploy.md` | Pi install beside the live Python unit — do not steal `:8080` |

---

## Meta — when to update this file

- A locked decision changes → **`docs/CHARTER.md` first** (dated amendment), then the summary here
- A gotcha is discovered → **`docs/gotchas.md`**, not here
- A slice completes → tick it in `BACKLOG.md`
- A doc file is created → add a row to `## Docs`
- **Keep this file under ~250 lines / ~20 KB.** Claude Code warns on oversized CLAUDE.md and
  it loads into every session's context. Fat goes to `docs/`; this file points.
- Before publishing the repo, inline anything it truly depends on from `Launchpad-RS/` so the
  repo stands alone for outside readers.

### What never goes in CLAUDE.md or docs/*.md

- Task progress, session logs, completed-work summaries → Cerebro (`session_save`)
- Git SHAs, version pins → stale in days, belong in git history
- Commentary on what you just did → belongs in commit messages
- **Credentials of any kind** → env files (0600, root-owned), never a tracked file
