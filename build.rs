use embed_manifest::{embed_manifest, manifest::ExecutionLevel, new_manifest};

fn main() {
    // Pede permissao de administrador ao abrir (necessario para parar servicos)
    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        embed_manifest(
            new_manifest("WinOpti").requested_execution_level(ExecutionLevel::RequireAdministrator),
        )
        .expect("falha ao embutir manifesto");
    }
    println!("cargo:rerun-if-changed=build.rs");
}
