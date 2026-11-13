use std::env;
use std::path::PathBuf;
use std::process::Command;

fn run(command: &mut Command) {
    let status = command.status().expect("failed to run native build tool");
    assert!(status.success(), "native build tool failed: {command:?}");
}

fn main() {
    println!("cargo:rerun-if-changed=src/nucleus/linux/wayland_native.c");
    println!("cargo:rerun-if-changed=src/nucleus/linux/generated/xdg-shell-protocol.c");
    println!("cargo:rerun-if-changed=src/nucleus/linux/generated/xdg-shell-client-protocol.h");
    println!("cargo:rerun-if-changed=src/nucleus/linux/generated/xdg-decoration-protocol.c");
    println!("cargo:rerun-if-changed=src/nucleus/linux/generated/xdg-decoration-client-protocol.h");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("linux") {
        return;
    }

    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set"));
    let cc = env::var_os("CC").unwrap_or_else(|| "cc".into());
    let ar = env::var_os("AR").unwrap_or_else(|| "ar".into());
    let source_dir = PathBuf::from("src/nucleus/linux");
    let generated_dir = source_dir.join("generated");
    let wrapper_object = out.join("wayland_native.o");
    let protocol_object = out.join("xdg-shell-protocol.o");
    let decoration_object = out.join("xdg-decoration-protocol.o");
    let archive = out.join("libvagus_wayland.a");

    for (source, object) in [
        (source_dir.join("wayland_native.c"), wrapper_object),
        (generated_dir.join("xdg-shell-protocol.c"), protocol_object),
        (
            generated_dir.join("xdg-decoration-protocol.c"),
            decoration_object,
        ),
    ] {
        let mut compile = Command::new(&cc);
        compile
            .arg("-std=c11")
            .arg("-fPIC")
            .arg("-I")
            .arg(&generated_dir)
            .arg("-c")
            .arg(source)
            .arg("-o")
            .arg(object);
        run(&mut compile);
    }
    run(Command::new(ar)
        .arg("crus")
        .arg(&archive)
        .arg(out.join("wayland_native.o"))
        .arg(out.join("xdg-shell-protocol.o"))
        .arg(out.join("xdg-decoration-protocol.o")));

    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=vagus_wayland");
    println!("cargo:rustc-link-lib=wayland-client");
}
