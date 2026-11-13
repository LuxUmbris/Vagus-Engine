use vagus::api::window::{Window, WindowCfg};

#[test]
fn default_config_and_dimension_validation_are_consistent() {
    let cfg = WindowCfg::default();
    assert!(cfg.visible);
    assert!(!cfg.focused && !cfg.minimized && !cfg.maximized && !cfg.should_close);

    assert!(Window::create_window("invalid-size", 0, 240).is_err());
}

#[cfg(target_os = "linux")]
#[test]
fn apply_cfg_reaches_available_linux_backends() {
    use vagus::api::window::{Window, WindowCfg};

    let Ok(mut api_window) = Window::create_window("api-window-cfg", 320, 240) else {
        return;
    };
    assert_eq!(api_window.size, (320, 240));
    assert_eq!(api_window.framebuffer_size, (320, 240));
    assert_eq!(api_window.scale_factor, 1.0);
    assert_ne!(api_window.id.get(), 0);

    let fixed = WindowCfg {
        fullscreen: false,
        resizable: false,
        borderless: false,
        ..WindowCfg::default()
    };
    api_window.apply_cfg(fixed).unwrap();
    let mut unsupported = fixed;
    unsupported.focused = true;
    assert!(api_window.apply_cfg(unsupported).is_err());
    assert_eq!(api_window.cfg, fixed);
    api_window.apply_cfg(WindowCfg::default()).unwrap();
    api_window.poll_events().unwrap();

    if let Ok(mut x11) =
        vagus::nucleus::linux::x11::Window::create_window("x11-window-cfg", 320, 240)
    {
        x11.apply_cfg(false, false, true).unwrap();
        x11.apply_cfg(false, true, false).unwrap();
    }

    if let Ok(mut wayland) =
        vagus::nucleus::linux::wayland::Window::create_window("wayland-window-cfg", 320, 240)
    {
        wayland.apply_cfg(false, false, false).unwrap();
        wayland.apply_cfg(true, true, false).unwrap();
        wayland.apply_cfg(false, true, false).unwrap();
        if let Err(error) = wayland.apply_cfg(false, true, true) {
            assert!(error.contains("does not support borderless"));
        }
    }
}

#[cfg(target_os = "windows")]
#[test]
fn apply_cfg_changes_windows_styles_and_fullscreen() {
    let mut window = vagus::nucleus::windows::Window::create_window("windows-window-cfg", 320, 240)
        .expect("create Windows window");
    window.apply_cfg(false, false, true).unwrap();
    window.apply_cfg(true, false, false).unwrap();
    window.apply_cfg(false, true, false).unwrap();
}
