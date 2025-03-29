use std::{
    env,
    fs::File,
    io::{BufWriter, Write},
    path::PathBuf,
};

/// Generate the `device.x` linker script
/// as required by `cortex-m-rt` for
/// interrupt vector default handlers.
pub fn build(interrupt_idents: &[&str]) {
    let out = &PathBuf::from(env::var_os("OUT_DIR").unwrap());

    let mut writer = BufWriter::new(File::create(out.join("device.x")).unwrap());

    for vector in interrupt_idents {
        writeln!(writer, "PROVIDE({} = DefaultHandler);", vector).unwrap();
    }

    println!("cargo:rustc-link-search={}", out.display());
}
