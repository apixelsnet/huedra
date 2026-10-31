//! Compiles the Slint user interface when building for Windows, with the
//! Fluent style (light or dark, following Windows).

use std::env;
use std::process;

fn main() {
    println!("cargo:rerun-if-changed=ui");
    let windows = env::var("CARGO_CFG_TARGET_OS").is_ok_and(|os| os == "windows");
    if !windows {
        return;
    }
    let config = slint_build::CompilerConfiguration::new().with_style("fluent".into());
    if let Err(error) = slint_build::compile_with_config("ui/app.slint", config) {
        eprintln!("{error}");
        process::exit(1);
    }
}
