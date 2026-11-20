use nucleus::{TextureDevice, validate_rgba8_texture_data};

#[test]
fn rgba8_texture_validation_checks_nonzero_extent_and_exact_size() {
    assert!(validate_rgba8_texture_data(2, 1, &[0; 8]).is_ok());
    assert!(validate_rgba8_texture_data(0, 1, &[]).is_err());
    assert!(validate_rgba8_texture_data(1, 1, &[0; 3]).is_err());
    assert!(validate_rgba8_texture_data(u32::MAX, u32::MAX, &[]).is_err());
}

#[test]
fn texture_device_uploads_and_owns_a_vulkan_image_when_available() {
    let device = match TextureDevice::new_standalone() {
        Ok(device) => device,
        Err(error) => {
            eprintln!("skipping Vulkan texture upload: {error}");
            return;
        }
    };
    let pixels = [12, 34, 56, 255, 78, 90, 123, 255];
    let texture = device
        .create_rgba8_texture(2, 1, &pixels)
        .expect("create Vulkan texture");

    drop(device);
    assert_eq!(texture.width(), 2);
    assert_eq!(texture.height(), 1);
}
