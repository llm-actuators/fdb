interface Bounds {
  x: number;
  y: number;
  w: number;
  h: number;
}

interface SemanticFont {
  family: string;
  weight: string;
  size: number;
}

interface Padding {
  top: number;
  bottom: number;
  start: number;
  end: number;
}

interface Icon {
  name: string;
  format: string;
  paths: string[];
}

interface SemanticElement {
  id: string;
  platform_id?: string;
  type: string;
  content?: string;
  font?: SemanticFont;
  color?: string;
  bounds: Bounds;
  clickable: boolean;
  background?: string;
  corner_radius?: number;
  padding?: Padding;
  icon?: Icon;
  children?: SemanticElement[];
}

interface SemanticSchema {
  screen: string;
  device: string;
  platform: string;
  timestamp: string;
  viewport?: { width: number; height: number; density: number };
  elements: SemanticElement[];
}

figma.showUI(__html__, { visible: false, width: 0, height: 0 });

figma.ui.onmessage = (msg: { type: string }) => {
  if (msg.type === "copy-done") {
    figma.notify("YAML copied to clipboard");
  }
};

if (figma.command === "export-selection") {
  exportSelection();
} else if (figma.command === "export-page") {
  exportPage();
}

function exportSelection(): void {
  const sel = figma.currentPage.selection;
  if (sel.length === 0) {
    figma.notify("Select a frame first");
    figma.closePlugin();
    return;
  }
  const node = sel[0];
  if (!("children" in node)) {
    figma.notify("Select a frame or group");
    figma.closePlugin();
    return;
  }
  const schema = extractSchema(node as FrameNode);
  outputSchema(schema);
}

function exportPage(): void {
  const page = figma.currentPage;
  const frames = page.children.filter(
    (n): n is FrameNode => n.type === "FRAME"
  );
  if (frames.length === 0) {
    figma.notify("No frames on this page");
    figma.closePlugin();
    return;
  }
  const schema = extractSchema(frames[0]);
  outputSchema(schema);
}

function extractSchema(root: FrameNode | ComponentNode | InstanceNode | GroupNode): SemanticSchema {
  const originX = root.absoluteTransform[0][2];
  const originY = root.absoluteTransform[1][2];

  const elements: SemanticElement[] = [];
  walkNode(root, originX, originY, elements);

  const schema: SemanticSchema = {
    screen: root.name,
    device: "figma",
    platform: "figma",
    timestamp: new Date().toISOString(),
    elements,
  };

  if ("width" in root && "height" in root) {
    schema.viewport = {
      width: Math.round(root.width),
      height: Math.round(root.height),
      density: 1,
    };
  }

  return schema;
}

function walkNode(
  node: SceneNode,
  originX: number,
  originY: number,
  elements: SemanticElement[]
): void {
  if (!node.visible) return;

  if (!("absoluteTransform" in node)) {
    if ("children" in node) {
      for (const child of (node as ChildrenMixin & SceneNode).children) {
        walkNode(child, originX, originY, elements);
      }
    }
    return;
  }

  const lnode = node as SceneNode & { absoluteTransform: Transform; width: number; height: number };
  const x = Math.round(lnode.absoluteTransform[0][2] - originX);
  const y = Math.round(lnode.absoluteTransform[1][2] - originY);
  const w = Math.round(lnode.width);
  const h = Math.round(lnode.height);
  const bounds: Bounds = { x, y, w, h };

  const elemType = classifyType(node);
  const content = node.type === "TEXT" ? (node as TextNode).characters : undefined;

  const id = slugify(content ?? node.name);
  const platformId = node.id;

  const font = extractFont(node);
  const color = extractTextColor(node);
  const background = node.type !== "TEXT" ? extractFillColor(node) : undefined;
  const cornerRadius = extractCornerRadius(node);
  const padding = extractPadding(node);
  const icon = extractIcon(node);

  const skip = elemType === "container" && !content && w > 300 && h > 300;

  if (!skip) {
    const elem: SemanticElement = {
      id,
      platform_id: platformId,
      type: elemType,
      bounds,
      clickable: false,
    };
    if (content !== undefined) elem.content = content;
    if (font) elem.font = font;
    if (color) elem.color = color;
    if (background) elem.background = background;
    if (cornerRadius !== undefined) elem.corner_radius = cornerRadius;
    if (padding) elem.padding = padding;
    if (icon) elem.icon = icon;
    elements.push(elem);
  }

  if ("children" in node) {
    for (const child of (node as ChildrenMixin & SceneNode).children) {
      walkNode(child, originX, originY, elements);
    }
  }
}

function classifyType(node: SceneNode): string {
  switch (node.type) {
    case "TEXT":
      return "text";
    case "RECTANGLE":
    case "ELLIPSE":
    case "LINE":
    case "VECTOR":
    case "STAR":
    case "POLYGON":
      return "image";
    case "INSTANCE":
    case "COMPONENT":
    case "COMPONENT_SET":
      return hasTextChild(node as ChildrenMixin & SceneNode) ? "button" : "container";
    case "FRAME":
    case "GROUP":
    case "SECTION":
      return "container";
    default:
      return "view";
  }
}

function hasTextChild(node: ChildrenMixin & SceneNode): boolean {
  if (!("children" in node)) return false;
  for (const child of node.children) {
    if (child.type === "TEXT") return true;
    if ("children" in child && hasTextChild(child as ChildrenMixin & SceneNode)) return true;
  }
  return false;
}

function extractFont(node: SceneNode): SemanticFont | undefined {
  if (node.type !== "TEXT") return undefined;
  const text = node as TextNode;
  const family = text.fontName;
  if (family === figma.mixed) return undefined;
  const size = text.fontSize;
  if (size === figma.mixed) return undefined;
  const weight = text.fontWeight;
  if (weight === figma.mixed) return undefined;

  return {
    family: (family as FontName).family.toLowerCase(),
    weight: weightName(typeof weight === "number" ? weight : 400),
    size: typeof size === "number" ? size : 0,
  };
}

function extractTextColor(node: SceneNode): string | undefined {
  if (node.type !== "TEXT") return undefined;
  return extractFillColor(node);
}

function extractFillColor(node: SceneNode): string | undefined {
  if (!("fills" in node)) return undefined;
  const fills = (node as GeometryMixin & SceneNode).fills;
  if (fills === figma.mixed || !Array.isArray(fills)) return undefined;
  for (const fill of fills) {
    if (!fill.visible) continue;
    if (fill.type === "SOLID") {
      const r = Math.round(fill.color.r * 255);
      const g = Math.round(fill.color.g * 255);
      const b = Math.round(fill.color.b * 255);
      return `#${hex(r)}${hex(g)}${hex(b)}`;
    }
  }
  return undefined;
}

function extractCornerRadius(node: SceneNode): number | undefined {
  if (!("cornerRadius" in node)) return undefined;
  const r = (node as RectangleNode).cornerRadius;
  if (r === figma.mixed || r === 0) return undefined;
  return typeof r === "number" ? r : undefined;
}

function extractPadding(node: SceneNode): Padding | undefined {
  if (!("paddingTop" in node)) return undefined;
  const f = node as FrameNode;
  if (
    f.paddingTop === 0 &&
    f.paddingBottom === 0 &&
    f.paddingLeft === 0 &&
    f.paddingRight === 0
  )
    return undefined;
  return {
    top: f.paddingTop,
    bottom: f.paddingBottom,
    start: f.paddingLeft,
    end: f.paddingRight,
  };
}

function extractIcon(node: SceneNode): Icon | undefined {
  if (node.type !== "VECTOR" && node.type !== "STAR" && node.type !== "POLYGON")
    return undefined;
  const vnode = node as VectorNode;
  const paths: string[] = [];
  if (vnode.vectorPaths) {
    for (const vp of vnode.vectorPaths) {
      if (vp.data) paths.push(vp.data);
    }
  }
  if (paths.length === 0) return undefined;
  return {
    name: slugify(node.name),
    format: "svg_path",
    paths,
  };
}

function weightName(w: number): string {
  if (w < 150) return "thin";
  if (w < 250) return "extralight";
  if (w < 350) return "light";
  if (w < 450) return "regular";
  if (w < 550) return "medium";
  if (w < 650) return "semibold";
  if (w < 750) return "bold";
  if (w < 850) return "extrabold";
  return "black";
}

function slugify(s: string): string {
  return s
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "_")
    .replace(/^_|_$/g, "")
    .replace(/_+/g, "_");
}

function hex(n: number): string {
  return n.toString(16).padStart(2, "0").toUpperCase();
}

function toYaml(schema: SemanticSchema): string {
  let out = "";
  out += `screen: ${yamlStr(schema.screen)}\n`;
  out += `device: ${yamlStr(schema.device)}\n`;
  out += `platform: ${yamlStr(schema.platform)}\n`;
  out += `timestamp: ${yamlStr(schema.timestamp)}\n`;
  if (schema.viewport) {
    out += "viewport:\n";
    out += `  width: ${schema.viewport.width}\n`;
    out += `  height: ${schema.viewport.height}\n`;
    out += `  density: ${schema.viewport.density}\n`;
  }
  out += "elements:\n";
  for (const el of schema.elements) {
    out += elementToYaml(el, 0);
  }
  return out;
}

function elementToYaml(el: SemanticElement, indent: number): string {
  const pad = "  ".repeat(indent);
  let out = `${pad}- id: ${yamlStr(el.id)}\n`;
  if (el.platform_id !== undefined)
    out += `${pad}  platform_id: ${yamlStr(el.platform_id)}\n`;
  out += `${pad}  type: ${yamlStr(el.type)}\n`;
  if (el.content !== undefined)
    out += `${pad}  content: ${yamlStr(el.content)}\n`;
  if (el.font) {
    out += `${pad}  font:\n`;
    out += `${pad}    family: ${yamlStr(el.font.family)}\n`;
    out += `${pad}    weight: ${yamlStr(el.font.weight)}\n`;
    out += `${pad}    size: ${el.font.size}\n`;
  }
  if (el.color !== undefined) out += `${pad}  color: ${yamlStr(el.color)}\n`;
  out += `${pad}  bounds:\n`;
  out += `${pad}    x: ${el.bounds.x}\n`;
  out += `${pad}    y: ${el.bounds.y}\n`;
  out += `${pad}    w: ${el.bounds.w}\n`;
  out += `${pad}    h: ${el.bounds.h}\n`;
  out += `${pad}  clickable: ${el.clickable}\n`;
  if (el.background !== undefined)
    out += `${pad}  background: ${yamlStr(el.background)}\n`;
  if (el.corner_radius !== undefined)
    out += `${pad}  corner_radius: ${el.corner_radius}\n`;
  if (el.padding) {
    out += `${pad}  padding:\n`;
    out += `${pad}    top: ${el.padding.top}\n`;
    out += `${pad}    bottom: ${el.padding.bottom}\n`;
    out += `${pad}    start: ${el.padding.start}\n`;
    out += `${pad}    end: ${el.padding.end}\n`;
  }
  if (el.icon) {
    out += `${pad}  icon:\n`;
    out += `${pad}    name: ${yamlStr(el.icon.name)}\n`;
    out += `${pad}    format: ${yamlStr(el.icon.format)}\n`;
    if (el.icon.paths.length > 0) {
      out += `${pad}    paths:\n`;
      for (const p of el.icon.paths) {
        out += `${pad}    - ${yamlStr(p)}\n`;
      }
    }
  }
  if (el.children && el.children.length > 0) {
    out += `${pad}  children:\n`;
    for (const child of el.children) {
      out += elementToYaml(child, indent + 2);
    }
  }
  return out;
}

function yamlStr(s: string): string {
  if (/^[a-zA-Z0-9_./#-]+$/.test(s) && !s.includes(": ")) return s;
  return `"${s.replace(/\\/g, "\\\\").replace(/"/g, '\\"').replace(/\n/g, "\\n")}"`;
}

function outputSchema(schema: SemanticSchema): void {
  const yaml = toYaml(schema);
  console.log(yaml);
  figma.ui.show();
  figma.ui.postMessage({ type: "copy", text: yaml });
  figma.notify(`Exported ${schema.elements.length} elements from "${schema.screen}"`);
}
