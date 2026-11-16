use vagus::window::{Window, WindowCfg};

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
    let mut window = match Window::create_window("api-window-cfg", 320, 240) {
        Ok(window) => window,
        Err(error) => {
            eprintln!("skipping native window assertions: {error}");
            return;
        }
    };
    assert_eq!(window.size, (320, 240));
    assert_eq!(window.framebuffer_size, (320, 240));
    assert_eq!(window.scale_factor, 1.0);
    assert_ne!(window.id.get(), 0);
    assert_eq!(window.id(), window.id);
    assert!(window.is_open());

    let fixed = WindowCfg {
        fullscreen: false,
        resizable: false,
        borderless: false,
        ..WindowCfg::default()
    };
    window.apply_cfg(fixed).unwrap();
    let mut focus_request = fixed;
    focus_request.focused = true;
    let focus_result = window.apply_cfg(focus_request);
    if focus_result.is_err() {
        assert_eq!(window.cfg, fixed);
        assert!(window.set_visible(false).is_err());
        assert!(window.set_size(400, 300).is_err());
        assert!(window.set_position(20, 30).is_err());
        assert!(window.request_focus().is_err());
    } else {
        assert!(window.cfg.focused);
        window.set_visible(false).unwrap();
        window.set_visible(true).unwrap();
        window.set_size(400, 300).unwrap();
        window.set_position(20, 30).unwrap();
        window.request_focus().unwrap();
    }
    let mut maximized = fixed;
    maximized.maximized = true;
    window.apply_cfg(maximized).unwrap();
    assert!(window.cfg.maximized);
    window.apply_cfg(fixed).unwrap();
    assert!(!window.cfg.maximized);
    let conflicting_state = WindowCfg {
        minimized: true,
        maximized: true,
        ..fixed
    };
    assert!(window.apply_cfg(conflicting_state).is_err());
    assert_eq!(window.cfg, fixed);
    window.minimize().unwrap();
    assert!(window.cfg.minimized);
    window.maximize().unwrap();
    assert!(window.cfg.maximized);
    window.restore().unwrap();
    assert!(!window.cfg.minimized && !window.cfg.maximized);
    window.set_title("api-window-updated").unwrap();
    assert_eq!(window.title, "api-window-updated");
    window.apply_cfg(WindowCfg::default()).unwrap();
    window.poll_events().unwrap();
    window.request_close();
    assert!(!window.is_open());
}

#[cfg(target_os = "windows")]
#[test]
fn apply_cfg_changes_windows_styles_and_fullscreen() {
    let mut window = match Window::create_window("windows-window-cfg", 320, 240) {
        Ok(window) => window,
        Err(error) => {
            eprintln!("skipping native window assertions: {error}");
            return;
        }
    };
    window
        .apply_cfg(WindowCfg {
            resizable: false,
            borderless: true,
            ..WindowCfg::default()
        })
        .unwrap();
    window
        .apply_cfg(WindowCfg {
            fullscreen: true,
            resizable: false,
            borderless: false,
            ..WindowCfg::default()
        })
        .unwrap();
    window.apply_cfg(WindowCfg::default()).unwrap();
}
