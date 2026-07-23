# figma-debug-bridge (fdb)

Rust CLI that extracts UI semantic schema from Figma designs. Pulls a frame via the Figma REST API (or converts an offline JSON dump, or decodes a Kiwi WebSocket scenegraph capture) and emits YAML in the same format that `ddb` / `idb` produce from live devices. That symmetry is what makes the cross-platform drift pipeline (Epic E Phase 4) work: every consumer sees the same schema regardless of source.

End-to-end verified 2026-06-10 against an example Figma file (`<FILE_KEY>` node `<NODE_ID>` → 235-element extract; schema byte-equivalent to the existing catalogue captures).

## Install

```bash
cd figma-debug-bridge/fdb
cargo build --release
# Use `install` (atomic tmp+rename) instead of `cp` — overwriting a
# running binary can break macOS codesign cache and trigger SIGKILL
# (TD-125).
install -m 755 target/release/fdb /opt/homebrew/bin/fdb
```

## Quick start

```bash
# Extract a Figma frame, write semantic.yaml
fdb ui --semantic \
       --file <FILE_KEY> \
       --node <NODE_ID> \
       --output /tmp/frame.yaml

# Drop straight into a catalogue tree (refresh recipe)
fdb ui --semantic \
       --file <FILE_KEY> \
       --node <NODE_ID> \
       --output catalogue/figma/<screen>/semantic.yaml
```

`tctl figma-refresh` wraps this loop over `tctl/figma-frames.toml` so individual screens don't need ad-hoc invocations.

## Auth

`fdb` resolves the Figma API token from, in order:

1. `--token <…>` flag
2. `FIGMA_TOKEN` environment variable
3. `~/.config/substrate/<company>/secrets.properties` (`FIGMA_TOKEN=…` line)

When multiple tokens are listed in the secrets file and one returns HTTP 429, fdb rotates to the next. Responses are cached at `~/.cache/fdb/<file_key>/<node_id>/response.json` and revalidated against the `lastModified` metadata.

## Commands

Detailed command reference (flags, kiwi decoder, asset extraction) lives in [`docs/README.md`](docs/README.md). Headline commands:

| Command | Purpose |
|---|---|
| `fdb ui --file --node` | Fetch a Figma frame via REST API + extract semantic schema |
| `fdb convert -i <input> [--kiwi]` | Convert offline Figma JSON or Kiwi scenegraph capture to semantic YAML |
| `fdb convert --dump-assets <dir>` | Extract image assets referenced by a frame to a directory |

## Repo layout

```
figma-debug-bridge/
├── fdb/                 # Rust binary crate (the CLI itself)
│   ├── src/
│   ├── Cargo.toml
│   └── target/
├── plugin/              # Figma plugin (TypeScript) — semantic schema export sidecar
│   ├── src/
│   ├── dist/
│   ├── manifest.json
│   └── package.json
├── docs/README.md       # Full command reference
└── README.md            # This file
```

## Cross-links

- `tctl/docs/epic-e-phase4-design.md` §1 — pipeline stages (fdb extract → vdb element-matrix → tctl drift-report → gate).
- `tctl/docs/adr/ADR-012-figma-drift-pre-commit-gate.md` D4 — figma-as-reference severity asymmetry (defines fdb's output role: source-of-truth side of the diff).
- `tctl/docs/adr/ADR-013-drift-check-single-verb.md` — how `tctl drift-check --with-figma-refresh` invokes fdb under the covers.
- `semantic-schema/` — the canonical Rust types fdb's output conforms to.

## Part of substrate-distro

Sibling repos in the toolchain:

- `ddb` — Android device + test runner (produces matching semantic schema from live devices)
- `idb` — iOS device + test runner (produces matching semantic schema from sims/devices)
- `vdb` — Visual diff (consumes the (figma, android, ios) triplet via `vdb element-matrix`)
- `semantic-schema` — Canonical schema crate
- `tctl` — Toolchain control + documentation root (`tctl/docs/llms.txt` is the canonical index)
