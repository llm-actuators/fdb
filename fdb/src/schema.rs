use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct SemanticSchema {
    pub screen: String,
    pub device: String,
    pub platform: String,
    pub timestamp: String,
    pub elements: Vec<SemanticElement>,
}

#[derive(Debug, Serialize)]
pub struct SemanticElement {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform_id: Option<String>,
    #[serde(rename = "type")]
    pub elem_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font: Option<Font>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    pub bounds: Bounds,
    pub clickable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub corner_radius: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub padding: Option<Padding>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<Icon>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub render: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<SemanticElement>>,
}

#[derive(Debug, Serialize)]
pub struct Bounds {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

#[derive(Debug, Serialize)]
pub struct Font {
    pub family: String,
    pub weight: String,
    pub size: f64,
}

#[derive(Debug, Serialize)]
pub struct Padding {
    pub top: i32,
    pub bottom: i32,
    pub start: i32,
    pub end: i32,
}

#[derive(Debug, Serialize)]
pub struct Icon {
    pub name: String,
    pub format: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<String>,
}
