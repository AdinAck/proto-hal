pub mod macros;
pub mod render;

use std::{collections::HashMap, env, fs, path::Path};

use model::{diagnostic, structures::model::Model};

fn generate(hal: &Model, output: impl FnOnce(&Model) -> Result<HashMap<String, String>, String>) {
    let out_dir = env::var("OUT_DIR").unwrap();

    let diagnostics = hal.validate();

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

    let Ok(codegen) = output(hal) else {
        println!("cargo::error=Codegen failed. Refer to the model crate for details.");
        return;
    };

    for (path, contents) in codegen {
        let dest_path = Path::new(&out_dir).join(path);
        fs::write(&dest_path, contents).unwrap();
    }

    println!("cargo:rustc-link-search={out_dir}");
}
