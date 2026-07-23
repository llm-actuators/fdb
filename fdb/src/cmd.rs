use clap::{Args, Parser, Subcommand};

use crate::figma;
use crate::kiwi;

#[derive(Parser)]
#[command(name = "fdb", version, about = "Figma Debug Bridge — extract semantic schema from Figma")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Extract semantic schema from a Figma frame
    Ui(UiArgs),
    /// Convert Figma JSON (file or stdin) to semantic YAML
    Convert(ConvertArgs),
}

#[derive(Args)]
pub struct UiArgs {
    /// Semantic extraction mode
    #[arg(long)]
    pub semantic: bool,

    /// Figma file key
    #[arg(long)]
    pub file: String,

    /// Figma node ID (frame to extract)
    #[arg(long)]
    pub node: String,

    /// Output file path
    #[arg(short, long)]
    pub output: Option<String>,

    /// Figma API token (defaults to FIGMA_TOKEN env or secrets.properties)
    #[arg(long)]
    pub token: Option<String>,
}

#[derive(Args)]
pub struct ConvertArgs {
    /// Input JSON file path, or "-" for stdin
    #[arg(short, long, default_value = "-")]
    pub input: String,

    /// Output file path (defaults to stdout)
    #[arg(short, long)]
    pub output: Option<String>,

    /// Screen name override
    #[arg(long)]
    pub screen: Option<String>,

    /// Dump vector SVGs and collect image refs to this directory
    #[arg(long)]
    pub dump_assets: Option<String>,

    /// Figma file key (for fetching images via REST API)
    #[arg(long)]
    pub file_key: Option<String>,

    /// Figma API token (for image fetching)
    #[arg(long)]
    pub token: Option<String>,

    /// Input is kiwi scenegraph format (nodeChanges array)
    #[arg(long)]
    pub kiwi: bool,

    /// Extract a specific frame by name from the kiwi tree
    #[arg(long)]
    pub frame: Option<String>,
}

pub fn run(cli: Cli) -> Result<(), String> {
    match cli.command {
        Command::Ui(args) => run_ui(args),
        Command::Convert(args) => run_convert(args),
    }
}

fn run_ui(args: UiArgs) -> Result<(), String> {
    let tokens = if let Some(ref t) = args.token {
        vec![t.clone()]
    } else {
        load_all_tokens()
    };

    if tokens.is_empty() {
        return Err("no Figma token. set FIGMA_TOKEN env, --token flag, or add to secrets.properties".to_string());
    }

    let mut last_err = String::new();
    let mut resp = None;
    for (i, token) in tokens.iter().enumerate() {
        match figma::fetch_nodes(token, &args.file, &args.node) {
            Ok(r) => {
                resp = Some(r);
                break;
            }
            Err(e) => {
                if e.contains("429") && i + 1 < tokens.len() {
                    eprintln!("token {} rate limited, trying next", i + 1);
                    continue;
                }
                last_err = e;
            }
        }
    }

    let resp = resp.ok_or(last_err)?;

    let node = resp
        .nodes
        .values()
        .next()
        .ok_or_else(|| "no nodes returned from Figma API".to_string())?;

    let schema = figma::extract_schema(&node.document, None);

    let yaml = serde_yaml::to_string(&schema).map_err(|e| format!("yaml error: {e}"))?;

    if let Some(ref path) = args.output {
        std::fs::write(path, &yaml).map_err(|e| format!("write error: {e}"))?;
        eprintln!("wrote {}", path);
    } else {
        print!("{yaml}");
    }

    Ok(())
}

fn run_convert(args: ConvertArgs) -> Result<(), String> {
    let json = if args.input == "-" {
        use std::io::Read;
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .map_err(|e| format!("stdin read error: {e}"))?;
        buf
    } else {
        std::fs::read_to_string(&args.input)
            .map_err(|e| format!("read {}: {e}", args.input))?
    };

    let node = if args.kiwi {
        let doc: kiwi::KiwiDocument = serde_json::from_str(&json)
            .map_err(|e| format!("kiwi parse error: {e}"))?;
        eprintln!("{} nodeChanges parsed", doc.node_changes.len());
        let mut tree = kiwi::convert_to_figma_tree(&doc);

        // If --frame specified, find that child frame
        if let Some(ref frame_name) = args.frame {
            tree = find_frame(&tree, frame_name)
                .ok_or_else(|| format!("frame '{}' not found in kiwi tree", frame_name))?;
        }
        tree
    } else if let Ok(resp) = serde_json::from_str::<figma::FileNodesResponse>(&json) {
        let wrapper = resp
            .nodes
            .into_values()
            .next()
            .ok_or("no nodes in FileNodesResponse")?;
        wrapper.document
    } else {
        serde_json::from_str::<figma::FigmaNode>(&json)
            .map_err(|e| format!("JSON is neither FileNodesResponse nor FigmaNode: {e}"))?
    };

    let mut assets = args.dump_assets.as_ref().map(|dir| {
        figma::AssetContext::new(std::path::PathBuf::from(dir))
    });

    let mut schema = figma::extract_schema_with_assets(&node, None, assets.as_mut());
    if let Some(name) = args.screen {
        schema.screen = name;
    }

    // Fetch images via REST API if we have image refs and credentials
    if let Some(ref ctx) = assets {
        if !ctx.image_refs.is_empty() {
            let tokens = if let Some(ref t) = args.token {
                vec![t.clone()]
            } else {
                load_all_tokens()
            };
            if let Some(ref file_key) = args.file_key {
                for token in &tokens {
                    match figma::fetch_images(token, file_key, &ctx.image_refs, &ctx.dir) {
                        Ok(n) => {
                            eprintln!("{n} images fetched to {}", ctx.dir.display());
                            break;
                        }
                        Err(e) => eprintln!("image fetch error: {e}"),
                    }
                }
            } else {
                eprintln!(
                    "{} image refs collected but no --file-key provided for REST fetch",
                    ctx.image_refs.len()
                );
            }
        }

        let svg_count = std::fs::read_dir(&ctx.dir)
            .map(|rd| rd.filter_map(|e| e.ok()).filter(|e| {
                e.path().extension().map_or(false, |ext| ext == "svg")
            }).count())
            .unwrap_or(0);
        if svg_count > 0 {
            eprintln!("{svg_count} SVGs saved to {}", ctx.dir.display());
        }
    }

    let yaml = serde_yaml::to_string(&schema).map_err(|e| format!("yaml error: {e}"))?;

    if let Some(ref path) = args.output {
        std::fs::write(path, &yaml).map_err(|e| format!("write error: {e}"))?;
        eprintln!("wrote {}", path);
    } else {
        print!("{yaml}");
    }

    Ok(())
}

fn load_all_tokens() -> Vec<String> {
    let mut tokens = Vec::new();

    // Env vars first
    if let Ok(t) = std::env::var("FIGMA_TOKEN") {
        tokens.push(t);
    }
    if let Ok(t) = std::env::var("FIGMA_TOKEN_2") {
        tokens.push(t);
    }

    // Then secrets files under ~/.config/substrate/<company>/secrets.properties.
    // Company is taken from $SUBSTRATE_COMPANY when set; otherwise every
    // direct subdirectory of ~/.config/substrate is scanned.
    let home = match dirs_next::home_dir() {
        Some(h) => h,
        None => return tokens,
    };
    let base = home.join(".config/substrate");
    let candidates: Vec<std::path::PathBuf> = match std::env::var("SUBSTRATE_COMPANY") {
        Ok(name) if !name.is_empty() => vec![base.join(name).join("secrets.properties")],
        _ => match std::fs::read_dir(&base) {
            Ok(entries) => entries
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
                .map(|e| e.path().join("secrets.properties"))
                .collect(),
            Err(_) => Vec::new(),
        },
    };
    for path in candidates {
        if let Ok(content) = std::fs::read_to_string(path) {
            for line in content.lines() {
                if let Some(rest) = line.strip_prefix("FIGMA_TOKEN=") {
                    let t = rest.trim().to_string();
                    if !tokens.contains(&t) {
                        tokens.push(t);
                    }
                }
                if let Some(rest) = line.strip_prefix("FIGMA_TOKEN_2=") {
                    let t = rest.trim().to_string();
                    if !tokens.contains(&t) {
                        tokens.push(t);
                    }
                }
            }
        }
    }

    tokens
}

fn find_frame(node: &figma::FigmaNode, name: &str) -> Option<figma::FigmaNode> {
    if node.name.eq_ignore_ascii_case(name) && matches!(node.node_type.as_str(), "FRAME" | "COMPONENT" | "INSTANCE") {
        return Some(node.clone());
    }
    if let Some(ref children) = node.children {
        for child in children {
            if let Some(found) = find_frame(child, name) {
                return Some(found);
            }
        }
    }
    None
}
