use serde::{Deserialize, Deserializer};

use crate::schema::*;

#[derive(Debug, Clone, Deserialize)]
pub struct FileNodesResponse {
    pub nodes: std::collections::HashMap<String, NodeWrapper>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NodeWrapper {
    pub document: FigmaNode,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FigmaNode {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub node_type: String,
    #[serde(default)]
    pub visible: Option<bool>,
    #[serde(default)]
    pub characters: Option<String>,
    #[serde(default)]
    pub style: Option<TypeStyle>,
    #[serde(default)]
    pub fills: Option<Vec<Paint>>,
    #[serde(default)]
    pub strokes: Option<Vec<Paint>>,
    #[serde(default)]
    pub corner_radius: Option<f64>,
    #[serde(default)]
    pub absolute_bounding_box: Option<BoundingBox>,
    #[serde(default)]
    pub padding_left: Option<f64>,
    #[serde(default)]
    pub padding_right: Option<f64>,
    #[serde(default)]
    pub padding_top: Option<f64>,
    #[serde(default)]
    pub padding_bottom: Option<f64>,
    #[serde(default)]
    pub item_spacing: Option<f64>,
    #[serde(default)]
    pub fill_geometry: Option<Vec<VectorPath>>,
    #[serde(default)]
    pub stroke_geometry: Option<Vec<VectorPath>>,
    #[serde(default)]
    pub children: Option<Vec<FigmaNode>>,
    #[serde(default)]
    pub component_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeStyle {
    pub font_family: Option<String>,
    pub font_weight: Option<f64>,
    pub font_size: Option<f64>,
    pub letter_spacing: Option<f64>,
    pub line_height_px: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Paint {
    #[serde(rename = "type")]
    pub paint_type: Option<String>,
    pub color: Option<FigmaColor>,
    pub visible: Option<bool>,
    pub opacity: Option<f64>,
    pub image_ref: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VectorPath {
    pub path: String,
    #[serde(default)]
    pub winding_rule: Option<String>,
}

fn f64_or_zero<'de, D: Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
    Ok(Option::<f64>::deserialize(d)?.unwrap_or(0.0))
}

#[derive(Debug, Clone, Deserialize)]
pub struct FigmaColor {
    #[serde(default, deserialize_with = "f64_or_zero")]
    pub r: f64,
    #[serde(default, deserialize_with = "f64_or_zero")]
    pub g: f64,
    #[serde(default, deserialize_with = "f64_or_zero")]
    pub b: f64,
    #[serde(default, deserialize_with = "f64_or_zero")]
    pub a: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BoundingBox {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

pub fn fetch_nodes(
    token: &str,
    file_key: &str,
    node_id: &str,
) -> Result<FileNodesResponse, String> {
    let cache = Cache::new(file_key, node_id);

    // Check cache freshness via lightweight file metadata endpoint
    if cache.exists() {
        match check_last_modified(token, file_key) {
            Ok(remote_modified) => {
                if let Some(cached_modified) = cache.read_meta() {
                    if cached_modified == remote_modified {
                        eprintln!("cache hit (lastModified unchanged)");
                        if let Some(cached) = cache.read_response() {
                            return Ok(cached);
                        }
                    }
                }
                // Cache stale — will re-fetch below
                eprintln!("cache stale (lastModified changed)");
            }
            Err(e) => {
                // Metadata check failed (rate limited?) — use cache if available
                eprintln!("metadata check failed: {e} — using cache");
                if let Some(cached) = cache.read_response() {
                    return Ok(cached);
                }
            }
        }
    }

    // Fetch from API
    let url = format!(
        "https://api.figma.com/v1/files/{}/nodes?ids={}",
        file_key, node_id
    );
    let client = reqwest::blocking::Client::new();
    let resp = client
        .get(&url)
        .header("X-Figma-Token", token)
        .send()
        .map_err(|e| format!("figma api error: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().unwrap_or_default();

        // If rate limited but we have a cache, return it
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            eprintln!("rate limited — checking cache");
            if let Some(cached) = cache.read_response() {
                return Ok(cached);
            }
        }

        return Err(format!("figma api {status}: {body}"));
    }

    let body = resp.text().map_err(|e| format!("read body: {e}"))?;

    // Save to cache
    if let Ok(modified) = check_last_modified(token, file_key) {
        cache.write(&body, &modified);
    } else {
        cache.write(&body, "unknown");
    }

    serde_json::from_str::<FileNodesResponse>(&body)
        .map_err(|e| format!("figma parse error: {e}"))
}

fn check_last_modified(token: &str, file_key: &str) -> Result<String, String> {
    let url = format!("https://api.figma.com/v1/files/{}", file_key);
    let client = reqwest::blocking::Client::new();
    let resp = client
        .get(&url)
        .header("X-Figma-Token", token)
        .send()
        .map_err(|e| format!("figma metadata error: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("figma metadata {}", resp.status()));
    }

    #[derive(serde::Deserialize)]
    struct FileMeta {
        #[serde(rename = "lastModified")]
        last_modified: String,
    }

    let meta: FileMeta = resp.json().map_err(|e| format!("parse metadata: {e}"))?;
    Ok(meta.last_modified)
}

struct Cache {
    dir: std::path::PathBuf,
}

impl Cache {
    fn new(file_key: &str, node_id: &str) -> Self {
        let safe_node = node_id.replace(':', "_");
        let dir = dirs_next::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
            .join(".cache/fdb")
            .join(file_key)
            .join(safe_node);
        Self { dir }
    }

    fn exists(&self) -> bool {
        self.dir.join("response.json").exists()
    }

    fn read_meta(&self) -> Option<String> {
        std::fs::read_to_string(self.dir.join("meta.txt")).ok()
    }

    fn read_response(&self) -> Option<FileNodesResponse> {
        let body = std::fs::read_to_string(self.dir.join("response.json")).ok()?;
        serde_json::from_str(&body).ok()
    }

    fn write(&self, body: &str, last_modified: &str) {
        if let Err(e) = std::fs::create_dir_all(&self.dir) {
            eprintln!("cache mkdir error: {e}");
            return;
        }
        let _ = std::fs::write(self.dir.join("response.json"), body);
        let _ = std::fs::write(self.dir.join("meta.txt"), last_modified);
        eprintln!("cached to {}", self.dir.display());
    }
}

pub struct AssetContext {
    pub dir: std::path::PathBuf,
    pub image_refs: Vec<(String, String)>, // (node_id, image_hash)
}

impl AssetContext {
    pub fn new(dir: std::path::PathBuf) -> Self {
        let _ = std::fs::create_dir_all(&dir);
        Self {
            dir,
            image_refs: Vec::new(),
        }
    }

    fn save_svg(&self, name: &str, node: &FigmaNode, bbox: &BoundingBox) -> Option<String> {
        let fill_paths = node.fill_geometry.as_ref();
        let stroke_paths = node.stroke_geometry.as_ref();
        let has_fills = fill_paths.map_or(false, |v| !v.is_empty());
        let has_strokes = stroke_paths.map_or(false, |v| !v.is_empty());
        if !has_fills && !has_strokes {
            return None;
        }

        let w = bbox.width;
        let h = bbox.height;
        let fill_color = extract_fill_color(&node.fills).unwrap_or_else(|| "#000000".to_string());
        let stroke_color = extract_fill_color(&node.strokes).unwrap_or_else(|| "#000000".to_string());

        let mut svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}">"#,
        );

        if let Some(paths) = fill_paths {
            for p in paths {
                svg.push_str(&format!(
                    r#"<path d="{}" fill="{}" fill-rule="{}"/>"#,
                    p.path,
                    fill_color,
                    p.winding_rule.as_deref().unwrap_or("nonzero").to_lowercase()
                ));
            }
        }
        if let Some(paths) = stroke_paths {
            for p in paths {
                svg.push_str(&format!(
                    r#"<path d="{}" fill="none" stroke="{}"/>"#,
                    p.path, stroke_color
                ));
            }
        }

        svg.push_str("</svg>");

        let filename = format!("{name}.svg");
        let path = self.dir.join(&filename);
        if std::fs::write(&path, &svg).is_ok() {
            Some(format!("assets/{filename}"))
        } else {
            None
        }
    }

    fn record_image_ref(&mut self, node_id: &str, name: &str, fills: &Option<Vec<Paint>>) -> Option<String> {
        let fills = fills.as_ref()?;
        for fill in fills {
            if fill.visible == Some(false) {
                continue;
            }
            if let Some(ref hash) = fill.image_ref {
                let filename = format!("{name}.png");
                self.image_refs.push((node_id.to_string(), hash.clone()));
                return Some(format!("assets/{filename}"));
            }
        }
        None
    }
}

pub fn fetch_images(
    token: &str,
    file_key: &str,
    image_refs: &[(String, String)],
    asset_dir: &std::path::Path,
) -> Result<usize, String> {
    if image_refs.is_empty() {
        return Ok(0);
    }

    let node_ids: Vec<&str> = image_refs.iter().map(|(id, _)| id.as_str()).collect();
    let ids_param = node_ids.join(",");

    let url = format!(
        "https://api.figma.com/v1/images/{}?ids={}&format=png&scale=2",
        file_key, ids_param
    );
    let client = reqwest::blocking::Client::new();
    let resp = client
        .get(&url)
        .header("X-Figma-Token", token)
        .send()
        .map_err(|e| format!("figma images api: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("figma images api {}", resp.status()));
    }

    #[derive(serde::Deserialize)]
    struct ImagesResponse {
        images: std::collections::HashMap<String, Option<String>>,
    }

    let data: ImagesResponse = resp.json().map_err(|e| format!("parse images: {e}"))?;

    let mut count = 0;
    for (node_id, _hash) in image_refs {
        if let Some(Some(url)) = data.images.get(node_id) {
            let img_resp = client.get(url).send().ok();
            if let Some(r) = img_resp {
                if r.status().is_success() {
                    let bytes = r.bytes().unwrap_or_default();
                    let safe_id = node_id.replace(':', "_");
                    let path = asset_dir.join(format!("{safe_id}.png"));
                    if std::fs::write(&path, &bytes).is_ok() {
                        count += 1;
                    }
                }
            }
        }
    }

    Ok(count)
}

pub fn extract_schema(
    node: &FigmaNode,
    frame_box: Option<&BoundingBox>,
) -> SemanticSchema {
    extract_schema_with_assets(node, frame_box, None)
}

pub fn extract_schema_with_assets(
    node: &FigmaNode,
    frame_box: Option<&BoundingBox>,
    assets: Option<&mut AssetContext>,
) -> SemanticSchema {
    let frame = node.absolute_bounding_box.as_ref().or(frame_box);
    let origin_x = frame.map(|b| b.x).unwrap_or(0.0);
    let origin_y = frame.map(|b| b.y).unwrap_or(0.0);

    let mut elements = Vec::new();
    walk_node(node, origin_x, origin_y, &mut elements, assets);

    SemanticSchema {
        screen: node.name.clone(),
        device: "figma".to_string(),
        platform: "figma".to_string(),
        timestamp: chrono_now(),
        elements,
    }
}

fn walk_node(
    node: &FigmaNode,
    origin_x: f64,
    origin_y: f64,
    elements: &mut Vec<SemanticElement>,
    mut assets: Option<&mut AssetContext>,
) {
    if node.visible == Some(false) {
        return;
    }

    let bbox = match &node.absolute_bounding_box {
        Some(b) => b,
        None => {
            if let Some(children) = &node.children {
                for child in children {
                    walk_node(child, origin_x, origin_y, elements, assets.as_mut().map(|a| &mut **a));
                }
            }
            return;
        }
    };

    let bounds = Bounds {
        x: (bbox.x - origin_x).round() as i32,
        y: (bbox.y - origin_y).round() as i32,
        w: bbox.width.round() as i32,
        h: bbox.height.round() as i32,
    };

    let elem_type = classify_figma_type(&node.node_type, node.characters.is_some());
    let content = node.characters.clone();

    let id = if let Some(ref c) = content {
        slugify(c)
    } else {
        slugify(&node.name)
    };

    let platform_id = Some(node.id.clone());

    let font = node.style.as_ref().and_then(|s| {
        let family = s.font_family.as_deref()?;
        Some(Font {
            family: family.to_lowercase(),
            weight: weight_name(s.font_weight.unwrap_or(400.0)),
            size: s.font_size.unwrap_or(0.0),
        })
    });

    let color = extract_fill_color(&node.fills);
    let background = if node.node_type != "TEXT" {
        extract_fill_color(&node.fills)
    } else {
        None
    };

    let corner_radius = node.corner_radius;

    let padding = if node.padding_top.is_some()
        || node.padding_bottom.is_some()
        || node.padding_left.is_some()
        || node.padding_right.is_some()
    {
        Some(Padding {
            top: node.padding_top.unwrap_or(0.0) as i32,
            bottom: node.padding_bottom.unwrap_or(0.0) as i32,
            start: node.padding_left.unwrap_or(0.0) as i32,
            end: node.padding_right.unwrap_or(0.0) as i32,
        })
    } else {
        None
    };

    let is_vector = matches!(node.node_type.as_str(), "VECTOR" | "LINE" | "STAR" | "POLYGON" | "BOOLEAN_OPERATION");
    let has_image_fill = node.fills.as_ref().map_or(false, |fills| {
        fills.iter().any(|f| f.image_ref.is_some() && f.visible != Some(false))
    });

    let image_path = if let Some(ref mut ctx) = assets {
        if is_vector {
            ctx.save_svg(&id, node, bbox)
        } else if has_image_fill {
            ctx.record_image_ref(&node.id, &id, &node.fills)
        } else {
            None
        }
    } else {
        None
    };

    let skip = elem_type == "container" && content.is_none() && bounds.w > 300 && bounds.h > 300;

    if !skip {
        elements.push(SemanticElement {
            id,
            platform_id,
            elem_type,
            content,
            font,
            color,
            bounds,
            clickable: false,
            background,
            corner_radius,
            padding,
            icon: None,
            render: None,
            image_path,
            children: None,
        });
    }

    if let Some(children) = &node.children {
        for child in children {
            walk_node(child, origin_x, origin_y, elements,
                assets.as_mut().map(|a| &mut **a));
        }
    }
}

fn classify_figma_type(node_type: &str, has_text: bool) -> String {
    match node_type {
        "TEXT" => "text".to_string(),
        "RECTANGLE" | "ELLIPSE" | "LINE" | "VECTOR" => "image".to_string(),
        "INSTANCE" | "COMPONENT" | "COMPONENT_SET" => {
            if has_text {
                "button".to_string()
            } else {
                "container".to_string()
            }
        }
        "FRAME" | "GROUP" | "SECTION" => "container".to_string(),
        _ => "view".to_string(),
    }
}

fn extract_fill_color(fills: &Option<Vec<Paint>>) -> Option<String> {
    let fills = fills.as_ref()?;
    for fill in fills {
        if fill.visible == Some(false) {
            continue;
        }
        if let Some(ref color) = fill.color {
            let r = (color.r * 255.0).round() as u8;
            let g = (color.g * 255.0).round() as u8;
            let b = (color.b * 255.0).round() as u8;
            return Some(format!("#{:02X}{:02X}{:02X}", r, g, b));
        }
    }
    None
}

fn weight_name(w: f64) -> String {
    match w as u32 {
        0..=149 => "thin",
        150..=249 => "extralight",
        250..=349 => "light",
        350..=449 => "regular",
        450..=549 => "medium",
        550..=649 => "semibold",
        650..=749 => "bold",
        750..=849 => "extrabold",
        _ => "black",
    }
    .to_string()
}

fn slugify(s: &str) -> String {
    let slug: String = s
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    slug.split('_')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

fn chrono_now() -> String {
    use std::time::SystemTime;
    let dur = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = dur.as_secs();
    // Simple ISO 8601 without chrono dependency
    format!("{}Z", secs)
}
