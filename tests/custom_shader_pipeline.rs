//! Copyright © 2026 Vagus Engine Contributors
//! The authors of this file (the "Software") are not reliable
//! for any damages caused by the Software.
//! THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
//! NEITHER EXPRESS NOR IMPLIED. EVERY COPY OF THE SOFTWARE MUST
//! CONTAIN THIS TEXT. ANY MODIFICATIONS OF ANY KIND OF THE
//! SOFTWARE MUST BE EXPLICITLY STATED.
//!
//! For further information, see:
//! github.com/LuxUmbris/Vagus-Engine/LICENSE.
//!
//! Authors: Vincent Noll

use std::process::Command;
use std::sync::Mutex;
use vagus::math::Vec2;
use vagus::render::{Color, NamedColor, Renderer};
use vagus::shader::{Shader, ShaderStage};
use vagus::window::Window;

static GPU_TEST_LOCK: Mutex<()> = Mutex::new(());

struct ShaderFiles {
    source: std::path::PathBuf,
    output: std::path::PathBuf,
}

impl Drop for ShaderFiles {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.source);
        let _ = std::fs::remove_file(&self.output);
    }
}

fn constant_red_fragment_spirv() -> Vec<u8> {
    static NEXT_SHADER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let stem = format!(
        "vagus-render-fragment-{}-{}",
        std::process::id(),
        NEXT_SHADER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    );
    let files = ShaderFiles {
        source: std::env::temp_dir().join(format!("{stem}.frag")),
        output: std::env::temp_dir().join(format!("{stem}.spv")),
    };
    std::fs::write(
        &files.source,
        "#version 450\nlayout(location = 0) out vec4 color;\nvoid main() { color = vec4(1.0, 0.0, 0.0, 1.0); }\n",
    )
    .expect("write custom fragment shader source");
    let compiler =
        std::env::var_os("GLSLANG_VALIDATOR").unwrap_or_else(|| "glslangValidator".into());
    let result = Command::new(compiler)
        .args(["-V", "--target-env", "vulkan1.2", "-S", "frag", "-o"])
        .arg(&files.output)
        .arg(&files.source)
        .output()
        .expect("run GLSL compiler for custom fragment shader");
    assert!(
        result.status.success(),
        "custom fragment shader compilation failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    std::fs::read(&files.output).expect("read custom fragment SPIR-V")
}

fn red_fragment() -> Shader {
    Shader::from_spirv_bytes(
        ShaderStage::Fragment,
        &constant_red_fragment_spirv(),
        "main",
    )
    .expect("validate custom fragment shader")
}

fn constant_blue_vertex_spirv() -> Vec<u8> {
    static NEXT_SHADER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let stem = format!(
        "vagus-render-vertex-{}-{}",
        std::process::id(),
        NEXT_SHADER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    );
    let files = ShaderFiles {
        source: std::env::temp_dir().join(format!("{stem}.vert")),
        output: std::env::temp_dir().join(format!("{stem}.spv")),
    };
    std::fs::write(
        &files.source,
        "#version 450\nlayout(location = 0) in vec3 position;\nlayout(location = 2) in vec2 uv;\nlayout(location = 0) out vec4 vertex_color;\nlayout(location = 1) out vec2 vertex_uv;\nlayout(location = 2) out float vertex_lighting;\nvoid main() { gl_Position = vec4(position, 1.0); vertex_color = vec4(0.0, 0.0, 1.0, 1.0); vertex_uv = uv; vertex_lighting = 1.0; }\n",
    )
    .expect("write custom vertex shader source");
    let compiler =
        std::env::var_os("GLSLANG_VALIDATOR").unwrap_or_else(|| "glslangValidator".into());
    let result = Command::new(compiler)
        .args(["-V", "--target-env", "vulkan1.2", "-S", "vert", "-o"])
        .arg(&files.output)
        .arg(&files.source)
        .output()
        .expect("run GLSL compiler for custom vertex shader");
    assert!(
        result.status.success(),
        "custom vertex shader compilation failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    std::fs::read(&files.output).expect("read custom vertex SPIR-V")
}

fn blue_vertex() -> Shader {
    Shader::from_spirv_bytes(ShaderStage::Vertex, &constant_blue_vertex_spirv(), "main")
        .expect("validate custom vertex shader")
}

fn passthrough_fragment() -> Shader {
    static NEXT_SHADER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let stem = format!(
        "vagus-render-passthrough-{}-{}",
        std::process::id(),
        NEXT_SHADER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    );
    let files = ShaderFiles {
        source: std::env::temp_dir().join(format!("{stem}.frag")),
        output: std::env::temp_dir().join(format!("{stem}.spv")),
    };
    std::fs::write(
        &files.source,
        "#version 450\nlayout(location = 0) in vec4 vertex_color;\nlayout(location = 0) out vec4 color;\nvoid main() { color = vertex_color; }\n",
    )
    .expect("write passthrough fragment shader source");
    let compiler =
        std::env::var_os("GLSLANG_VALIDATOR").unwrap_or_else(|| "glslangValidator".into());
    let result = Command::new(compiler)
        .args(["-V", "--target-env", "vulkan1.2", "-S", "frag", "-o"])
        .arg(&files.output)
        .arg(&files.source)
        .output()
        .expect("run GLSL compiler for passthrough fragment shader");
    assert!(
        result.status.success(),
        "passthrough fragment shader compilation failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let bytes = std::fs::read(&files.output).expect("read passthrough fragment SPIR-V");
    Shader::from_spirv_bytes(ShaderStage::Fragment, &bytes, "main")
        .expect("validate passthrough fragment shader")
}

#[test]
fn renderer_uses_a_caller_loaded_fragment_shader_for_gpu_draws() {
    let _guard = GPU_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let fragment = red_fragment();
    let mut window = Window::create_window("vagus-custom-shader-render-test", 32, 32)
        .expect("create a native window for custom shader rendering");
    let mut renderer = Renderer::new(&mut window).expect("initialize Vulkan renderer");
    let module = renderer
        .shader_device()
        .load(&fragment)
        .expect("load fragment shader on the renderer's Vulkan device");
    renderer
        .set_fragment_shader(&module)
        .expect("create a graphics pipeline with the caller's fragment shader");
    renderer.draw_triangle_gpu(
        Vec2 { x: 4.0, y: 4.0 },
        Vec2 { x: 28.0, y: 4.0 },
        Vec2 { x: 16.0, y: 28.0 },
        NamedColor::Green,
    );
    let pixels = renderer
        .present_gpu_readback(Color::rgb(0, 0, 0))
        .expect("read back custom-shader frame");

    assert_eq!(pixels[12 * 32 + 16], [255, 0, 0, 255]);
    assert_eq!(pixels[0], [0, 0, 0, 255]);
}

#[test]
fn renderer_uses_a_caller_loaded_vertex_shader_for_gpu_draws() {
    let _guard = GPU_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let vertex = blue_vertex();
    let fragment = passthrough_fragment();
    let mut window = Window::create_window("vagus-custom-vertex-render-test", 32, 32)
        .expect("create a native window for custom vertex rendering");
    let mut renderer = Renderer::new(&mut window).expect("initialize Vulkan renderer");
    let shader_device = renderer.shader_device();
    let vertex_module = shader_device
        .load(&vertex)
        .expect("load vertex shader on the renderer's Vulkan device");
    let fragment_module = shader_device
        .load(&fragment)
        .expect("load fragment shader on the renderer's Vulkan device");
    renderer
        .set_vertex_shader(&vertex_module)
        .expect("create a graphics pipeline with the caller's vertex shader");
    renderer.draw_triangle_gpu(
        Vec2 { x: 4.0, y: 4.0 },
        Vec2 { x: 28.0, y: 4.0 },
        Vec2 { x: 16.0, y: 28.0 },
        NamedColor::Green,
    );
    let builtin_fragment_pixels = renderer
        .present_gpu_readback(Color::rgb(0, 0, 0))
        .expect("read back custom-vertex frame with the built-in fragment stage");
    assert_eq!(builtin_fragment_pixels[12 * 32 + 16], [0, 0, 255, 255]);

    renderer
        .set_fragment_shader(&fragment_module)
        .expect("preserve the custom vertex shader when installing the fragment shader");
    renderer.draw_triangle_gpu(
        Vec2 { x: 4.0, y: 4.0 },
        Vec2 { x: 28.0, y: 4.0 },
        Vec2 { x: 16.0, y: 28.0 },
        NamedColor::Green,
    );
    let pixels = renderer
        .present_gpu_readback(Color::rgb(0, 0, 0))
        .expect("read back custom-vertex frame");

    assert_eq!(pixels[12 * 32 + 16], [0, 0, 255, 255]);
    assert_eq!(pixels[0], [0, 0, 0, 255]);
}

#[test]
fn renderer_rejects_shader_modules_from_another_vulkan_device() {
    let _guard = GPU_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let fragment = red_fragment();
    let vertex = blue_vertex();
    let mut source_window = Window::create_window("vagus-source-shader-test", 32, 32)
        .expect("create a native window for source shader modules");
    let source_renderer =
        Renderer::new(&mut source_window).expect("initialize source Vulkan renderer");
    let source_device = source_renderer.shader_device();
    let fragment_module = source_device
        .load(&fragment)
        .expect("load fragment shader on source renderer device");
    let vertex_module = source_device
        .load(&vertex)
        .expect("load vertex shader on source renderer device");
    drop(source_renderer);
    drop(source_device);
    drop(source_window);

    let mut window = Window::create_window("vagus-foreign-shader-test", 32, 32)
        .expect("create a native window for device ownership test");
    let mut renderer = Renderer::new(&mut window).expect("initialize Vulkan renderer");

    let error = renderer
        .set_fragment_shader(&fragment_module)
        .expect_err("reject a shader module owned by another Vulkan device");
    assert!(error.contains("different Vulkan device"), "{error}");

    let error = renderer
        .set_vertex_shader(&vertex_module)
        .expect_err("reject a vertex module owned by another Vulkan device");
    assert!(error.contains("different Vulkan device"), "{error}");

    renderer.draw_triangle_gpu(
        Vec2 { x: 4.0, y: 4.0 },
        Vec2 { x: 28.0, y: 4.0 },
        Vec2 { x: 16.0, y: 28.0 },
        NamedColor::Green,
    );
    let pixels = renderer
        .present_gpu_readback(Color::rgb(0, 0, 0))
        .expect("read back frame after rejecting foreign shader modules");
    assert_eq!(pixels[12 * 32 + 16], [0, 255, 0, 255]);
    assert_eq!(pixels[0], [0, 0, 0, 255]);
}
