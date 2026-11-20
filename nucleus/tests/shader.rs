use nucleus::validate_spirv_words;

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
fn spirv_validation_rejects_bad_headers_and_truncated_instructions() {
    let valid = minimal_vertex_spirv();
    assert!(validate_spirv_words(&valid).is_ok());

    let mut bad_magic = valid.clone();
    bad_magic[0] = 0;
    assert!(validate_spirv_words(&bad_magic).is_err());

    let mut truncated_instruction = valid.clone();
    truncated_instruction[5] = 0x0003_0011;
    assert!(validate_spirv_words(&truncated_instruction).is_err());
}
