use serde::{Deserialize, Deserializer};
use std::collections::HashMap;

use crate::figma::{BoundingBox, FigmaNode, Paint, FigmaColor, TypeStyle, VectorPath};

fn nullable_f64<'de, D: Deserializer<'de>>(d: D) -> Result<Option<f64>, D::Error> {
    Ok(Option::<f64>::deserialize(d).unwrap_or(None))
}

fn f64_or_zero<'de, D: Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
    Ok(Option::<f64>::deserialize(d)?.unwrap_or(0.0))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KiwiDocument {
    #[serde(rename = "type")]
    pub doc_type: Option<String>,
    pub node_changes: Vec<KiwiNode>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KiwiNode {
    pub guid: KiwiGuid,
    #[serde(rename = "type")]
    pub node_type: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub visible: Option<bool>,
    #[serde(default)]
    pub opacity: Option<f64>,
    #[serde(default)]
    pub transform: Option<KiwiTransform>,
    #[serde(default)]
    pub size: Option<KiwiSize>,
    #[serde(default)]
    pub parent_index: Option<KiwiParentIndex>,
    #[serde(default)]
    pub fill_paints: Option<Vec<KiwiFill>>,
    #[serde(default)]
    pub stroke_paints: Option<Vec<KiwiFill>>,
    #[serde(default)]
    pub font_name: Option<KiwiFontName>,
    #[serde(default, deserialize_with = "nullable_f64")]
    pub font_size: Option<f64>,
    #[serde(default)]
    pub text_data: Option<KiwiTextData>,
    #[serde(default)]
    pub derived_text_data: Option<KiwiDerivedTextData>,
    #[serde(default)]
    pub corner_radius: Option<f64>,
    #[serde(default)]
    pub padding_left: Option<f64>,
    #[serde(default)]
    pub padding_right: Option<f64>,
    #[serde(default)]
    pub padding_top: Option<f64>,
    #[serde(default)]
    pub padding_bottom: Option<f64>,
    #[serde(default)]
    pub fill_geometry: Option<Vec<KiwiVectorPath>>,
    #[serde(default)]
    pub stroke_geometry: Option<Vec<KiwiVectorPath>>,
}

#[derive(Debug, Deserialize, Clone, PartialEq, Eq, Hash)]
pub struct KiwiGuid {
    #[serde(rename = "sessionID")]
    pub session_id: i64,
    #[serde(rename = "localID")]
    pub local_id: i64,
}

impl KiwiGuid {
    fn to_string(&self) -> String {
        format!("{}:{}", self.session_id, self.local_id)
    }
}

#[derive(Debug, Deserialize)]
pub struct KiwiTransform {
    #[serde(default, deserialize_with = "f64_or_zero")]
    pub m00: f64,
    #[serde(default, deserialize_with = "f64_or_zero")]
    pub m01: f64,
    #[serde(default, deserialize_with = "f64_or_zero")]
    pub m02: f64,
    #[serde(default, deserialize_with = "f64_or_zero")]
    pub m10: f64,
    #[serde(default, deserialize_with = "f64_or_zero")]
    pub m11: f64,
    #[serde(default, deserialize_with = "f64_or_zero")]
    pub m12: f64,
}

#[derive(Debug, Deserialize)]
pub struct KiwiSize {
    #[serde(default, deserialize_with = "f64_or_zero")]
    pub x: f64,
    #[serde(default, deserialize_with = "f64_or_zero")]
    pub y: f64,
}

#[derive(Debug, Deserialize)]
pub struct KiwiParentIndex {
    pub guid: KiwiGuid,
    #[serde(default)]
    pub position: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KiwiFill {
    #[serde(rename = "type")]
    pub fill_type: Option<String>,
    #[serde(default)]
    pub color: Option<FigmaColor>,
    #[serde(default)]
    pub opacity: Option<f64>,
    #[serde(default)]
    pub visible: Option<bool>,
    #[serde(default)]
    pub image_ref: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct KiwiFontName {
    pub family: String,
    #[serde(default)]
    pub style: Option<String>,
    #[serde(default)]
    pub postscript: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct KiwiTextData {
    #[serde(default)]
    pub characters: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KiwiDerivedTextData {
    #[serde(default)]
    pub font_meta_data: Option<Vec<KiwiFontMeta>>,
    #[serde(default)]
    pub truncation_start_index: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KiwiFontMeta {
    pub key: Option<KiwiFontName>,
    #[serde(default)]
    pub font_weight: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KiwiVectorPath {
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub winding_rule: Option<String>,
}

struct AbsolutePosition {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

pub fn convert_to_figma_tree(doc: &KiwiDocument) -> FigmaNode {
    let nodes = &doc.node_changes;

    // Index by guid
    let mut by_guid: HashMap<(i64, i64), usize> = HashMap::new();
    for (i, n) in nodes.iter().enumerate() {
        by_guid.insert((n.guid.session_id, n.guid.local_id), i);
    }

    // Build children map
    let mut children_map: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    for (i, n) in nodes.iter().enumerate() {
        if let Some(ref pi) = n.parent_index {
            let parent_key = (pi.guid.session_id, pi.guid.local_id);
            children_map.entry(parent_key).or_default().push(i);
        }
    }

    // Compute absolute positions (walk from root)
    let mut abs_pos: HashMap<usize, AbsolutePosition> = HashMap::new();
    // Find root (DOCUMENT node, usually guid 0:0)
    let root_idx = nodes.iter().position(|n| n.parent_index.is_none())
        .unwrap_or(0);
    compute_absolute_positions(nodes, root_idx, 0.0, 0.0, &children_map, &mut abs_pos);

    // Convert root to FigmaNode tree
    build_figma_node(nodes, root_idx, &children_map, &abs_pos)
}

fn compute_absolute_positions(
    nodes: &[KiwiNode],
    idx: usize,
    parent_x: f64,
    parent_y: f64,
    children_map: &HashMap<(i64, i64), Vec<usize>>,
    abs_pos: &mut HashMap<usize, AbsolutePosition>,
) {
    let node = &nodes[idx];

    let (tx, ty) = if let Some(ref t) = node.transform {
        (t.m02, t.m12)
    } else {
        (0.0, 0.0)
    };

    let abs_x = parent_x + tx;
    let abs_y = parent_y + ty;
    let (w, h) = if let Some(ref s) = node.size {
        (s.x, s.y)
    } else {
        (0.0, 0.0)
    };

    abs_pos.insert(idx, AbsolutePosition { x: abs_x, y: abs_y, w, h });

    let key = (node.guid.session_id, node.guid.local_id);
    if let Some(child_indices) = children_map.get(&key) {
        for &ci in child_indices {
            compute_absolute_positions(nodes, ci, abs_x, abs_y, children_map, abs_pos);
        }
    }
}

fn build_figma_node(
    nodes: &[KiwiNode],
    idx: usize,
    children_map: &HashMap<(i64, i64), Vec<usize>>,
    abs_pos: &HashMap<usize, AbsolutePosition>,
) -> FigmaNode {
    let n = &nodes[idx];
    let key = (n.guid.session_id, n.guid.local_id);

    let bbox = abs_pos.get(&idx).map(|p| BoundingBox {
        x: p.x,
        y: p.y,
        width: p.w,
        height: p.h,
    });

    let fills = n.fill_paints.as_ref().map(|fps| {
        fps.iter().map(|f| Paint {
            paint_type: f.fill_type.clone(),
            color: f.color.clone(),
            visible: f.visible,
            opacity: f.opacity,
            image_ref: f.image_ref.clone(),
        }).collect()
    });

    let strokes = n.stroke_paints.as_ref().map(|sps| {
        sps.iter().map(|f| Paint {
            paint_type: f.fill_type.clone(),
            color: f.color.clone(),
            visible: f.visible,
            opacity: f.opacity,
            image_ref: f.image_ref.clone(),
        }).collect()
    });

    let characters = n.text_data.as_ref().and_then(|td| td.characters.clone());

    let font_weight = n.derived_text_data.as_ref()
        .and_then(|dtd| dtd.font_meta_data.as_ref())
        .and_then(|fm| fm.first())
        .and_then(|m| m.font_weight);

    let style = n.font_name.as_ref().map(|fn_| TypeStyle {
        font_family: Some(fn_.family.clone()),
        font_weight: font_weight.or(style_to_weight(fn_.style.as_deref())),
        font_size: n.font_size,
        letter_spacing: None,
        line_height_px: None,
    });

    let fill_geometry = n.fill_geometry.as_ref().map(|fgs| {
        fgs.iter().filter_map(|fg| {
            fg.path.as_ref().map(|p| VectorPath {
                path: p.clone(),
                winding_rule: fg.winding_rule.clone(),
            })
        }).collect()
    });

    let stroke_geometry = n.stroke_geometry.as_ref().map(|sgs| {
        sgs.iter().filter_map(|sg| {
            sg.path.as_ref().map(|p| VectorPath {
                path: p.clone(),
                winding_rule: sg.winding_rule.clone(),
            })
        }).collect()
    });

    let children_nodes = children_map.get(&key).map(|indices| {
        indices.iter().map(|&ci| {
            build_figma_node(nodes, ci, children_map, abs_pos)
        }).collect()
    });

    FigmaNode {
        id: n.guid.to_string(),
        name: n.name.clone().unwrap_or_default(),
        node_type: n.node_type.clone().unwrap_or_else(|| "UNKNOWN".to_string()),
        visible: n.visible,
        characters,
        style,
        fills,
        strokes,
        corner_radius: n.corner_radius,
        absolute_bounding_box: bbox,
        padding_left: n.padding_left,
        padding_right: n.padding_right,
        padding_top: n.padding_top,
        padding_bottom: n.padding_bottom,
        fill_geometry,
        stroke_geometry,
        item_spacing: None,
        children: children_nodes,
        component_id: None,
    }
}

fn style_to_weight(style: Option<&str>) -> Option<f64> {
    match style? {
        "Thin" | "Hairline" => Some(100.0),
        "ExtraLight" | "UltraLight" => Some(200.0),
        "Light" => Some(300.0),
        "Regular" | "Normal" => Some(400.0),
        "Medium" => Some(500.0),
        "SemiBold" | "DemiBold" => Some(600.0),
        "Bold" => Some(700.0),
        "ExtraBold" | "UltraBold" => Some(800.0),
        "Black" | "Heavy" => Some(900.0),
        _ => None,
    }
}
