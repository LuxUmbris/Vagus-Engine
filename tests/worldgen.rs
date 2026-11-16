use vagus::heightmap::HeightMap;
use vagus::material::Material;
use vagus::proc_gen::{NoiseSettings, generate_heightmap, sample_fbm, sample_value_noise_2d};
use vagus::texture::Texture;

#[test]
fn heightmap_stores_samples_and_bilinearly_interpolates_grid_coordinates() {
    let mut map = HeightMap::new(2, 2, vec![0.0, 2.0, 4.0, 6.0]).unwrap();

    assert_eq!(map.width(), 2);
    assert_eq!(map.height(), 2);
    assert_eq!(map.get(1, 0), Some(2.0));
    assert_eq!(map.sample_bilinear(0.5, 0.5), Some(3.0));
    assert_eq!(map.sample_bilinear(1.0, 1.0), Some(6.0));
    assert_eq!(map.sample_bilinear(-0.1, 0.0), None);

    map.set(1, 1, 8.0).unwrap();
    assert_eq!(map.get(1, 1), Some(8.0));
    assert_eq!(map.min_max(), (0.0, 8.0));
    assert!(map.set(2, 0, 0.0).is_err());
    assert!(map.set(0, 0, f32::NAN).is_err());
}

#[test]
fn heightmap_construction_checks_dimensions_samples_and_custom_generation() {
    assert!(HeightMap::new(0, 1, vec![]).is_err());
    assert!(HeightMap::new(2, 2, vec![0.0; 3]).is_err());
    assert!(HeightMap::new(1, 1, vec![f32::INFINITY]).is_err());

    let map = HeightMap::from_fn(3, 2, |x, y| (x + y * 10) as f32).unwrap();
    assert_eq!(map.heights(), &[0.0, 1.0, 2.0, 10.0, 11.0, 12.0]);
}

#[test]
fn heightmap_try_from_fn_propagates_custom_generation_errors() {
    let result = HeightMap::try_from_fn(2, 2, |x, y| {
        if x == 1 && y == 0 {
            Err("custom generator failed".to_owned())
        } else {
            Ok((x + y) as f32)
        }
    });

    assert_eq!(result.unwrap_err(), "custom generator failed");
}

#[test]
fn material_owns_and_exposes_its_texture_and_heightmap() {
    let texture = Texture::from_rgba8(1, 1, &[20, 40, 60, 255]).unwrap();
    let heightmap = HeightMap::new(2, 2, vec![0.0, 1.0, 2.0, 3.0]).unwrap();
    let material = Material::new(texture, heightmap);

    assert_eq!(material.texture().width(), 1);
    assert_eq!(material.texture().rgba8(), &[20, 40, 60, 255]);
    assert_eq!(material.heightmap().get(1, 1), Some(3.0));

    let (texture, heightmap) = material.into_parts();
    assert_eq!(texture.height(), 1);
    assert_eq!(heightmap.width(), 2);
}

#[test]
fn procedural_noise_is_seeded_bounded_and_generates_heightmaps() {
    let settings = NoiseSettings {
        seed: 42,
        ..NoiseSettings::default()
    };
    let sample = sample_fbm(12.5, -3.25, settings).unwrap();
    assert_eq!(sample, sample_fbm(12.5, -3.25, settings).unwrap());
    assert!((-1.0..=1.0).contains(&sample));
    assert_ne!(
        sample,
        sample_fbm(
            12.5,
            -3.25,
            NoiseSettings {
                seed: 43,
                ..settings
            }
        )
        .unwrap()
    );

    let map = generate_heightmap(4, 3, settings).unwrap();
    let repeated = generate_heightmap(4, 3, settings).unwrap();
    assert_eq!(map, repeated);
    assert_eq!(map.width(), 4);
    assert_eq!(map.height(), 3);
    assert!(
        map.heights()
            .iter()
            .all(|height| (-1.0..=1.0).contains(height))
    );
}

#[test]
fn procedural_noise_rejects_invalid_settings_and_coordinates() {
    assert!(sample_fbm(f32::NAN, 0.0, NoiseSettings::default()).is_err());
    assert!(
        sample_fbm(
            0.0,
            0.0,
            NoiseSettings {
                octaves: 0,
                ..NoiseSettings::default()
            }
        )
        .is_err()
    );
    assert!(
        sample_fbm(
            0.0,
            0.0,
            NoiseSettings {
                scale: 0.0,
                ..NoiseSettings::default()
            }
        )
        .is_err()
    );
}

#[test]
fn procedural_noise_rejects_lacunarity_that_overflows_sampling_coordinates() {
    let settings = NoiseSettings {
        octaves: 32,
        lacunarity: f32::MAX,
        ..NoiseSettings::default()
    };
    assert!(sample_fbm(0.0, 0.0, settings).is_err());
    assert!(generate_heightmap(1, 1, settings).is_err());

    let value = sample_value_noise_2d(1.25, -9.5, 17).unwrap();
    assert_eq!(value, sample_value_noise_2d(1.25, -9.5, 17).unwrap());
    assert!((-1.0..=1.0).contains(&value));
}
