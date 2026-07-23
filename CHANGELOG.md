# Changelog

All notable changes to fdb (Figma Debug Bridge) are documented here.

## [Unreleased]

### Verified (Epic E Phase 4 Wave 5, 2026-06-10)
- **End-to-end probe against an example Figma file.** `fdb ui --semantic --file <FILE_KEY> --node <NODE_ID> --output /tmp/fdb-probe.yaml` → 40 KB / 235 elements in ~1 s. Top-level keys + element fields + element types all byte-equivalent to existing `catalogue/figma/site-detail/semantic.yaml`. Pipeline (`vdb element-matrix → tctl drift-report`) consumes the fresh output without any translation. Cache mkdir error in sandboxed contexts is non-fatal (output still written).
- **Auth path documented.** `FIGMA_TOKEN` env, or `~/.config/substrate/<company>/secrets.properties`, or `--token <…>`; multi-token rotation on HTTP 429.
- **Used by `tctl figma-refresh`** (`tctl 1df4f23`, TD-118): iterates `tctl/figma-frames.toml` and invokes `fdb ui --semantic ...` per screen, writing into `catalogue/figma/<screen>/semantic.yaml`. Placeholder node IDs (`TODO-*`) yield clean `[SKIP]` entries.

### Notes
- See `substrate-distro/tctl/docs/epic-e-phase4-design.md` §1 for fdb's role in the cross-platform drift pipeline.
- See `substrate-distro/tctl/docs/tech-debt.md` TD-118 for the figma-refresh recipe codification history.

## [v0.1.0] — initial — `4645427`

Initial publish of `fdb`:

- `fdb ui` — Figma REST API extraction with semantic YAML output, response caching, multi-token rotation.
- `fdb convert` — offline Figma JSON or Kiwi WebSocket scenegraph → semantic YAML.
- `fdb convert --dump-assets` — extract image assets from a frame.
- Companion Figma plugin (TypeScript, `plugin/`) for semantic export from inside the Figma editor.
