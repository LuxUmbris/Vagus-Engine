use vagus::texture::{Texture, TextureDevice};

#[test]
fn rgba8_texture_validates_and_preserves_pixels() {
    let pixels = [255, 0, 0, 255, 0, 255, 0, 128];
    let texture = Texture::from_rgba8(2, 1, &pixels).expect("valid 2x1 RGBA texture");

    assert_eq!(texture.width(), 2);
    assert_eq!(texture.height(), 1);
    assert_eq!(texture.rgba8(), &pixels);
}

#[test]
fn rgba8_texture_rejects_invalid_dimensions_and_payload_lengths() {
    assert!(Texture::from_rgba8(0, 1, &[]).is_err());
    assert!(Texture::from_rgba8(1, 1, &[0, 0, 0]).is_err());
    assert!(Texture::from_rgba8(u32::MAX, u32::MAX, &[]).is_err());
}

#[test]
fn texture_device_uploads_a_validated_rgba8_image_when_vulkan_is_available() {
    let device = match TextureDevice::new() {
        Ok(device) => device,
        Err(error) => {
            eprintln!("skipping Vulkan texture upload: {error}");
            return;
        }
    };
    let pixels = [12, 34, 56, 255, 78, 90, 123, 255];
    let texture = Texture::from_rgba8(2, 1, &pixels).unwrap();
    let gpu_texture = device.upload(&texture).expect("upload Vulkan texture");

    drop(device);
    assert_eq!(gpu_texture.width(), 2);
    assert_eq!(gpu_texture.height(), 1);
}
