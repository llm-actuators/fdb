# fdb — Figma Debug Bridge

Rust CLI that extracts semantic schema from Figma designs. Supports direct REST API fetch, offline JSON conversion, and Kiwi scenegraph decoding from WebSocket captures. Outputs YAML in the shared semantic schema format used by ddb (Android), idb (iOS), and vdb (visual diff) for cross-platform design-to-implementation comparison.

## Install

```bash
cd figma-debug-bridge/fdb
cargo build --release
# Use `install` (atomic tmp+rename) instead of `cp` — overwriting a
# running binary can break macOS codesign cache and trigger SIGKILL
# (TD-125).
install -m 755 target/release/fdb /opt/homebrew/bin/fdb
```

## Commands

### ui

Fetch a Figma frame via REST API and extract semantic schema.

```bash
fdb ui --file <file_key> --node <node_id> [--semantic] [-o output.yaml] [--token <token>]
```

| Flag | Description |
|------|-------------|
| `--file <key>` | Figma file key (required) |
| `--node <id>` | Figma node ID to extract (required) |
| `--semantic` | Enable semantic extraction mode |
| `-o, --output <path>` | Output file (default: stdout) |
| `--token <token>` | Figma API token (see Token Resolution) |

**Caching:** Responses are cached at `~/.cache/fdb/<file_key>/<node_id>/response.json`. On subsequent calls, fdb checks the `lastModified` timestamp via a lightweight metadata request and returns cached data if unchanged. Falls back to cache on rate limit (HTTP 429).

**Token rotation:** When multiple tokens are available and one returns HTTP 429, fdb automatically tries the next token.

**Example:**
```bash
fdb ui --file <FILE_KEY> --node "123:456" -o /tmp/figma-frame.yaml
```

### convert

Convert Figma JSON or Kiwi scenegraph data to semantic YAML. Accepts input from file or stdin.

```bash
fdb convert [-i input.json] [-o output.yaml] [--kiwi] [--frame <name>] [--screen <name>] [--dump-assets <dir>] [--file-key <key>] [--token <token>]
```

| Flag | Description |
|------|-------------|
| `-i, --input <path>` | Input file path, or `-` for stdin (default: stdin) |
| `-o, --output <path>` | Output file (default: stdout) |
| `--kiwi` | Input is Kiwi scenegraph format (nodeChanges array) |
| `--frame <name>` | Extract specific frame by name from Kiwi tree (case-insensitive) |
| `--screen <name>` | Override screen name in output |
| `--dump-assets <dir>` | Extract vector SVGs and collect image refs to directory |
| `--file-key <key>` | Figma file key for REST image fetch (used with --dump-assets) |
| `--token <token>` | Figma API token for image fetch |

**Input formats (auto-detected unless --kiwi):**
1. `FileNodesResponse` — REST API response wrapper with `nodes` map
2. `FigmaNode` — bare node tree (e.g., saved from Figma plugin)
3. `KiwiDocument` — flat nodeChanges array (requires `--kiwi` flag)

**Examples:**

```bash
# REST API JSON → YAML
fdb convert -i response.json -o /tmp/schema.yaml

# Pipe from stdin
cat frame.json | fdb convert -o /tmp/schema.yaml

# Kiwi scenegraph → extract specific frame
fdb convert --kiwi -i /tmp/scenegraph.json --frame "Site" -o /tmp/site.yaml

# With asset extraction
fdb convert -i frame.json --dump-assets /tmp/figma-assets/ --file-key <FILE_KEY>

# Override screen name
fdb convert --kiwi -i scenegraph.json --frame "Home" --screen "HomeScreen" -o /tmp/home.yaml
```

## Kiwi Adapter

The `--kiwi` flag enables conversion of Figma's internal Kiwi scenegraph format — a flat array of `nodeChanges` captured from WebSocket frames — into the nested FigmaNode tree that fdb's semantic extractor expects.

**Conversion pipeline:**
1. Parse flat `nodeChanges[]` array with GUID-based identity (`sessionID:localID`)
2. Build parent-child relationships from `parentIndex.guid` references
3. Compute absolute screen positions by walking the transform matrix chain (`m02` = x translation, `m12` = y translation)
4. Map Kiwi fields to FigmaNode equivalents:

| Kiwi | FigmaNode |
|------|-----------|
| `guid.sessionID:localID` | `id` |
| `size.x / size.y` | `absoluteBoundingBox.width / height` |
| `transform.m02 / m12` (accumulated) | `absoluteBoundingBox.x / y` |
| `fillPaints` | `fills` |
| `strokePaints` | `strokes` |
| `fontName.family` | `style.fontFamily` |
| `derivedTextData.fontMetaData[0].fontWeight` | `style.fontWeight` |
| `textData.characters` | `characters` |
| `fillGeometry / strokeGeometry` | `fillGeometry / strokeGeometry` |

**Usage in the full pipeline:**
```bash
wdb ws-capture → figma-kiwi-protocol decode → fdb convert --kiwi → semantic YAML → vdb diff
```

## Asset Extraction

The `--dump-assets` flag enables two forms of asset extraction:

**Vector SVGs:** Nodes with `fillGeometry` or `strokeGeometry` (VECTOR, LINE, STAR, POLYGON, BOOLEAN_OPERATION) are rendered as SVG files with correct viewBox, fill colors, stroke colors, and winding rules. Saved as `<dir>/<element_name>.svg`.

**Raster images:** Nodes with image fills (`imageRef` in paint data) have their references collected. If `--file-key` and `--token` are provided, fdb batch-fetches the images via `GET /v1/images/<file_key>?ids=<node_ids>&format=png&scale=2` and saves them as `<dir>/<node_id>.png`.

Without `--file-key`, image refs are listed as warnings but not fetched. SVGs are always generated regardless.

Extracted assets are referenced in the YAML output via the `image_path` field (e.g., `image_path: "assets/arrow_back.svg"`).

## Token Resolution

Tokens are resolved in order (first found wins):

1. `--token` flag
2. `FIGMA_TOKEN` environment variable
3. `FIGMA_TOKEN_2` environment variable
4. `~/.config/substrate/<company>/secrets.properties` — lines matching `FIGMA_TOKEN=<value>` or `FIGMA_TOKEN_2=<value>`

The `ui` command uses all available tokens for rotation on rate limit (429). The `convert` command uses tokens only for `--dump-assets` image fetching.

## Output Schema

All commands output YAML in the shared semantic schema format:

```yaml
screen: "Site"
device: figma
platform: figma
timestamp: "2026-05-23T00:15:00Z"
elements:
  - id: site_name_text
    platform_id: "42:1234"
    type: text
    content: "Example Trail"
    font:
      family: poppins
      weight: "600"
      size: 20.0
    color: "#08292F"
    bounds:
      x: 16
      y: 120
      w: 343
      h: 28
    clickable: false
    background: "#FFFFFF"
    corner_radius: 8.0
    padding:
      top: 12
      bottom: 12
      start: 16
      end: 16
    image_path: "assets/icon.svg"
```

**Element type classification:**
| Figma node type | Semantic type |
|-----------------|---------------|
| TEXT | text |
| RECTANGLE, ELLIPSE, LINE, VECTOR | image |
| INSTANCE/COMPONENT with text | button |
| INSTANCE/COMPONENT without text | container |
| FRAME, GROUP, SECTION | container |
| Other | view |

## Examples

**Extract from REST API:**
```bash
export FIGMA_TOKEN=figd_xxx
fdb ui --file <FILE_KEY> --node "2:100" -o /tmp/design.yaml
```

**Full Kiwi pipeline:**
```bash
# 1. Capture WebSocket frames from Figma
wdb ws-capture --url "https://www.figma.com/design/..." --duration 30

# 2. Decode kiwi binary frames
figma-kiwi-protocol decode /tmp/ws-capture/ -o /tmp/scenegraph.json

# 3. Convert to semantic YAML
fdb convert --kiwi -i /tmp/scenegraph.json --frame "Site" --dump-assets /tmp/assets/ -o /tmp/design.yaml

# 4. Compare design vs implementation
vdb diff /tmp/design.yaml /tmp/android-site.yaml
```

**Batch extract all frames:**
```bash
for frame in "Site" "Home" "Search" "Profile"; do
  fdb convert --kiwi -i /tmp/scenegraph.json --frame "$frame" -o "/tmp/figma-${frame,,}.yaml"
done
```
