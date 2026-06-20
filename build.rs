use embed_manifest::{embed_manifest, new_manifest};
use embed_manifest::manifest::ExecutionLevel;

fn main() {
    // Only embed a manifest when actually building for Windows.
    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        let manifest = new_manifest("ProcessLogger")
            // Request a UAC elevation prompt when launched (e.g. double-clicked).
            .requested_execution_level(ExecutionLevel::RequireAdministrator);
        embed_manifest(manifest).expect("failed to embed application manifest");
    }
    println!("cargo:rerun-if-changed=build.rs");
}
