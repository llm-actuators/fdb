mod cmd;
mod figma;
mod kiwi;
mod schema;

use clap::Parser;

const MANIFEST_JSON: &str = r#"{"schema":"actuators-manifest/v1","name":"fdb","version":"0.1.0","category":"visual","deps":[],"provides":["fdb-figma-v1"],"consumes":["vdb-region-v1"],"configs":[]}"#;

fn main() {
    if std::env::args().nth(1).as_deref() == Some("--manifest") {
        println!("{MANIFEST_JSON}");
        return;
    }
    let cli = cmd::Cli::parse();
    if let Err(e) = cmd::run(cli) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
