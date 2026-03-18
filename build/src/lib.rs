#![allow(clippy::large_enum_variant)] // not a fan

#[cfg(feature = "macros")]
pub mod macros;

use std::{collections::HashMap, env, fs, path::Path};

use model::{Model, diagnostic};

#[cfg(feature = "integrated")]
/// Generate and emit HAL code for use.
pub fn render(model: &Model) {
    generate(model, |model| {
        Ok([
            ("hal.rs".to_string(), model.render()?),
            ("device.x".to_string(), model.interrupts().device_x()),
        ]
        .into())
    });
}

#[cfg(feature = "integrated")]
fn generate(model: &Model, output: impl FnOnce(&Model) -> Result<HashMap<String, String>, String>) {
    let out_dir = env::var("OUT_DIR").unwrap();

    let diagnostics = model.validate();

    let warning_count = diagnostics
        .iter()
        .filter(|diagnostic| matches!(diagnostic.rank(), diagnostic::Rank::Warning))
        .count();

    let error_count = diagnostics
        .iter()
        .filter(|diagnostic| matches!(diagnostic.rank(), diagnostic::Rank::Error))
        .count();

    match (warning_count, error_count) {
        (_, 1..) => {
            println!("cargo::error=HAL generation failed. Refer to the model crate for details.");
            return;
        }
        (1.., _) => {
            println!(
                "cargo::error=HAL generation contains warnings. Refer to the model crate for details."
            );
            return;
        }
        (..) => {}
    }

    let Ok(codegen) = output(model) else {
        println!("cargo::error=Codegen failed. Refer to the model crate for details.");
        return;
    };

    for (path, contents) in codegen {
        let dest_path = Path::new(&out_dir).join(path);
        fs::write(&dest_path, contents).unwrap();
    }

    println!("cargo:rustc-link-search={out_dir}");
}
