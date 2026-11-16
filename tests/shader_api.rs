use vagus::shader::{Shader, ShaderDevice, ShaderStage};

fn minimal_vertex_spirv() -> Vec<u32> {
    vec![
        0x0723_0203,
        0x0001_0000,
        0,
        5,
        0, // Header
        0x0002_0011,
        1, // OpCapability Shader
        0x0003_000e,
        0,
        1, // OpMemoryModel Logical GLSL450
        0x0005_000f,
        0,
        1,
        0x6e69_616d,
        0, // OpEntryPoint Vertex %1 "main"
        0x0002_0013,
        2, // OpTypeVoid %2
        0x0003_0021,
        3,
        2, // OpTypeFunction %3 %2
        0x0005_0036,
        2,
        1,
        0,
        3, // OpFunction
        0x0002_00f8,
        4,           // OpLabel %4
        0x0001_00fd, // OpReturn
        0x0001_0038, // OpFunctionEnd
    ]
}

#[test]
fn shader_accepts_words_and_little_endian_bytes() {
    let words = minimal_vertex_spirv();
    let shader = Shader::from_spirv(ShaderStage::Vertex, &words, "main").unwrap();
    assert_eq!(shader.stage(), ShaderStage::Vertex);
    assert_eq!(shader.entry_point(), "main");
    assert_eq!(shader.words(), words);

    let bytes: Vec<_> = words.iter().flat_map(|word| word.to_le_bytes()).collect();
    let decoded = Shader::from_spirv_bytes(ShaderStage::Vertex, &bytes, "main").unwrap();
    assert_eq!(decoded.words(), words);
}

#[test]
fn shader_rejects_malformed_spirv_and_entry_points() {
    assert!(Shader::from_spirv(ShaderStage::Vertex, &[], "main").is_err());
    assert!(Shader::from_spirv(ShaderStage::Vertex, &[0, 1, 2, 3, 4], "main").is_err());
    assert!(Shader::from_spirv(ShaderStage::Vertex, &minimal_vertex_spirv()[..6], "main").is_err());
    assert!(Shader::from_spirv(ShaderStage::Vertex, &minimal_vertex_spirv(), "").is_err());
    assert!(Shader::from_spirv(ShaderStage::Vertex, &minimal_vertex_spirv(), "bad\0name").is_err());
    assert!(Shader::from_spirv_bytes(ShaderStage::Vertex, &[1, 2, 3], "main").is_err());
    assert!(Shader::from_spirv(ShaderStage::Vertex, &minimal_vertex_spirv(), "missing").is_err());
    assert!(Shader::from_spirv(ShaderStage::Fragment, &minimal_vertex_spirv(), "main").is_err());
}

#[test]
fn native_shader_device_creates_a_shader_module_when_available() {
    let shader = Shader::from_spirv(ShaderStage::Vertex, &minimal_vertex_spirv(), "main").unwrap();
    let device = match ShaderDevice::new() {
        Ok(device) => device,
        Err(error) => {
            eprintln!("skipping Vulkan shader-module creation: {error}");
            return;
        }
    };
    let module = device.load(&shader).expect("create Vulkan shader module");
    assert_eq!(module.stage(), ShaderStage::Vertex);
    assert_eq!(module.entry_point(), "main");
}
