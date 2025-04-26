use std::{env, fs, path::Path};

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("hal.rs");

    let Ok(hal) = model::generate() else {
        println!("cargo::error=HAL generation failed. Refer to the model crate for details.");
        return;
    };

    let Ok(codegen) = hal.render() else {
        println!("cargo::error=Codegen failed. Refer to the model crate for details.");
        return;
    };

    fs::write(&dest_path, codegen).unwrap();
}
