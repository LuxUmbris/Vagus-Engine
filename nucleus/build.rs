use std::env;
use std::path::PathBuf;
use std::process::Command;

fn run(command: &mut Command) {
    let status = command.status().expect("failed to run native build tool");
    assert!(status.success(), "native build tool failed: {command:?}");
}

fn main() {
    let manifest_dir =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest dir is set"));
    let shader_dir = manifest_dir.join("src/shaders");
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set"));
    let glslang_validator =
        env::var_os("GLSLANG_VALIDATOR").unwrap_or_else(|| "glslangValidator".into());
    println!("cargo:rerun-if-env-changed=GLSLANG_VALIDATOR");
    for (source, stage, name) in [
        ("raygen.rgen", "rgen", "raygen"),
        ("miss.rmiss", "rmiss", "miss"),
        ("hit.rchit", "rchit", "hit"),
        ("draw.vert", "vert", "draw.vert"),
        ("draw.frag", "frag", "draw.frag"),
    ] {
        let source = shader_dir.join(source);
        let output = out.join(format!("{name}.spv"));
        println!("cargo:rerun-if-changed={}", source.display());
        let status = Command::new(&glslang_validator)
            .args(["-V", "--target-env", "vulkan1.2", "-S", stage, "-o"])
            .arg(&output)
            .arg(&source)
            .status()
            .unwrap_or_else(|error| {
                panic!(
                    "failed to run GLSL compiler {:?}: {error}",
                    glslang_validator
                )
            });
        assert!(
            status.success(),
            "GLSL compiler failed for {} with {status}",
            source.display()
        );
    }

    let source_dir = manifest_dir.join("src/linux");
    let generated_dir = source_dir.join("test");
    for path in [
        source_dir.join("wayland_native.c"),
        generated_dir.join("xdg-shell-protocol.c"),
        generated_dir.join("xdg-shell-client-protocol.h"),
        generated_dir.join("xdg-decoration-protocol.c"),
        generated_dir.join("xdg-decoration-client-protocol.h"),
        generated_dir.join("text-input-unstable-v3-protocol.c"),
        generated_dir.join("text-input-unstable-v3-client-protocol.h"),
        generated_dir.join("text-input-unstable-v3.xml"),
    ] {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("linux") {
        return;
    }

    let cc = env::var_os("CC").unwrap_or_else(|| "cc".into());
    let ar = env::var_os("AR").unwrap_or_else(|| "ar".into());
    let wrapper_object = out.join("wayland_native.o");
    let protocol_object = out.join("xdg-shell-protocol.o");
    let decoration_object = out.join("xdg-decoration-protocol.o");
    let text_input_object = out.join("text-input-unstable-v3-protocol.o");
    let archive = out.join("libnucleus_wayland.a");

    for (source, object) in [
        (source_dir.join("wayland_native.c"), wrapper_object),
        (generated_dir.join("xdg-shell-protocol.c"), protocol_object),
        (
            generated_dir.join("xdg-decoration-protocol.c"),
            decoration_object,
        ),
        (
            generated_dir.join("text-input-unstable-v3-protocol.c"),
            text_input_object,
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
        .arg(out.join("xdg-decoration-protocol.o"))
        .arg(out.join("text-input-unstable-v3-protocol.o")));

    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=nucleus_wayland");
    println!("cargo:rustc-link-lib=wayland-client");
    println!("cargo:rustc-link-lib=xkbcommon");
}
