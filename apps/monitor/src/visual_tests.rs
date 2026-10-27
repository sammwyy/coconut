//! Render and interact with the actual native tree without changing host services.
#![allow(clippy::field_reassign_with_default)]
use super::*;
use creamui_core::{Point, Renderer};
use creamui_render::{Damage, DisplayList, Rasterizer, SceneRecorder};
use creamui_theme::ThemeProvider;

fn fixture() -> Snapshot {
    let mut s = Snapshot::default();
    s.hostname = "dandelion".into();
    s.os_name = "Dandelion Linux".into();
    s.architecture = "x86_64".into();
    s.kernel = "6.17.2-arch1-1".into();
    s.uptime = 183420;
    s.cpu.model = "Intel Core m3-8100Y CPU @ 1.10GHz".into();
    s.cpu.cores = 4;
    s.cpu.usage = 21.0;
    s.cpu.sampled = true;
    s.cpu.per_core = vec![25.0, 18.0, 30.0, 11.0];
    s.cpu.frequency_mhz = Some(880.0);
    s.cpu.load = (1.1, 0.8, 0.6);
    s.cpu.driver = "intel_pstate".into();
    s.cpu.governor = "powersave".into();
    s.memory.total = 32 * 1024_u64.pow(3);
    s.memory.available = 21 * 1024_u64.pow(3);
    s.memory.swap_total = 4 * 1024_u64.pow(3);
    s.memory.swap_free = 3 * 1024_u64.pow(3);
    for i in 0..500 {
        let mut p = Process::default();
        p.pid = 11000 + i;
        p.name = ["firefox", "code", "cterm", "cfiles", "pipewire"][i as usize % 5].into();
        p.command = format!(
            "/usr/bin/{} --profile /home/sammy/a-long-directory-with-ñ-and-東京 --process={i}",
            p.name
        );
        p.executable = format!("/usr/bin/{}", p.name);
        p.user = unsafe { libc::geteuid() }.to_string();
        p.memory = (8 + i as u64 % 20) * 1024_u64.pow(2);
        p.cpu = (i % 10) as f32 * 0.2;
        p.threads = 4;
        p.state = "S".into();
        p.io_available = true;
        p.read_rate = 1024.0 * (i % 5) as f32;
        p.write_rate = 512.0 * (i % 3) as f32;
        p.start_ticks = 100;
        s.processes.push(p);
    }
    for (name, title) in [
        ("firefox", "Firefox"),
        ("code", "Visual Studio Code"),
        ("cterm", "Terminal"),
        ("cfiles", "File Explorer"),
    ] {
        let processes: Vec<_> = s
            .processes
            .iter()
            .filter(|p| p.name == name)
            .cloned()
            .collect();
        s.applications.push(crate::applications::Application {
            id: format!("{name}.desktop"),
            name: title.into(),
            cpu: processes.iter().map(|p| p.cpu).sum(),
            memory: processes.iter().map(|p| p.memory).sum(),
            processes,
        });
    }
    let mut disk = crate::telemetry::Drive::default();
    disk.name = "nvme0n1".into();
    disk.model = "Samsung SSD 980 PRO".into();
    disk.kind = "SSD".into();
    disk.size = 1024_u64.pow(4);
    disk.read_rate = 2.1 * 1024.0 * 1024.0;
    disk.write_rate = 0.4 * 1024.0 * 1024.0;
    disk.usage = 8.0;
    disk.sampled = true;
    s.drives.push(disk);
    s.disks.push(system::Disk {
        mount: "/".into(),
        device: "/dev/nvme0n1p2".into(),
        total: 1024_u64.pow(4),
        available: 640 * 1024_u64.pow(3),
        filesystem: "btrfs".into(),
    });
    s.gpus.push(crate::telemetry::Gpu {
        id: "card1".into(),
        name: "Intel UHD Graphics 615".into(),
        driver: "i915".into(),
        usage: Some(6.0),
        memory_used: Some(256 * 1024_u64.pow(2)),
        memory_total: Some(2 * 1024_u64.pow(3)),
        temperature: Some(34.0),
    });
    for (name, address, rate) in [
        ("enp5s0", "192.0.2.10", 768.0 * 1024.0),
        ("wlan0", "198.51.100.10", 128.0 * 1024.0),
        ("lo", "127.0.0.1", 0.0),
    ] {
        s.network.push(system::Network {
            name: name.into(),
            addresses: vec![address.into()],
            mac: "00:11:22:33:44:55".into(),
            state: "up".into(),
            received: 4 * 1024_u64.pow(3),
            transmitted: 768 * 1024_u64.pow(2),
            receive_rate: rate,
            transmit_rate: rate / 8.0,
            speed_mbps: Some(1000),
            sampled: true,
        });
    }
    for i in 0..80 {
        s.sockets.push(system::Socket {
            protocol: "TCP".into(),
            local: format!(
                "{}:{}",
                if i < 60 {
                    "192.0.2.10"
                } else {
                    "198.51.100.10"
                },
                50000 + i
            ),
            remote: "203.0.113.1:443".into(),
            state: "ESTABLISHED".into(),
            inode: (6000 + i).to_string(),
        });
    }
    s.sockets.push(system::Socket {
        protocol: "TCP".into(),
        local: "0.0.0.0:8080".into(),
        remote: "0.0.0.0:0".into(),
        state: "LISTEN".into(),
        inode: "7777".into(),
    });
    s.sockets.push(system::Socket {
        protocol: "UDP".into(),
        local: "127.0.0.1:53".into(),
        remote: "0.0.0.0:0".into(),
        state: "CLOSE".into(),
        inode: "7778".into(),
    });
    for i in 0..60 {
        s.services.push(system::Service {
            unit: format!("sample-{i}.service"),
            state: if i % 3 == 0 {
                "inactive / dead"
            } else {
                "active / running"
            }
            .into(),
            description: format!("Background service {i} with a descriptive name"),
        });
        s.user_services.push(system::Service {
            unit: format!("user-{i}.service"),
            state: "active / running".into(),
            description: format!("User service {i}"),
        });
    }
    s.temperatures.push(system::Temperature {
        label: "CPU package".into(),
        celsius: 34.0,
    });
    s
}
fn state() -> State {
    let state = State::new(fixture());
    state.histories.update(|histories| {
        for (key, values) in histories.iter_mut() {
            let last = values.last().copied().unwrap_or(0.0);
            *values = (0..30)
                .map(|i| {
                    let factor = (i as f32 * 0.7).sin() * 0.4 + 0.65;
                    if key == "memory" {
                        34.0 + factor
                    } else {
                        last * factor
                    }
                })
                .collect();
        }
    });
    state
}
fn frame(
    renderer: &mut Renderer,
    state: &State,
    router: &Router,
    theme: Theme,
    size: Size,
) -> (creamui_core::Scene, DisplayList) {
    creamui_reactive::provide_context(ThemeProvider::new(theme));
    let mut recorder = SceneRecorder::new();
    recorder.begin(
        size.width as u32,
        size.height as u32,
        1.0,
        CLEAR,
        theme.colors,
    );
    let root = RouterProvider::new(router.clone()).render(|| shell(size, state));
    let scene = renderer.render(root, size, &mut recorder);
    (scene, recorder.finish())
}
fn save(name: &str, list: &DisplayList) {
    if let Ok(directory) = std::env::var("CMONITOR_PREVIEW_DIR") {
        std::fs::create_dir_all(&directory).unwrap();
        let mut raster = Rasterizer::new(list.width, list.height);
        raster.render(list, &Damage::Full);
        raster
            .pixmap()
            .save_png(format!("{directory}/{name}.png"))
            .unwrap();
    }
}
fn top_pixels(list: &DisplayList, height: usize) -> Vec<u8> {
    let mut raster = Rasterizer::new(list.width, list.height);
    raster.render(list, &Damage::Full);
    raster.pixmap().data()[..list.width as usize * height * 4].to_vec()
}

#[test]
fn every_tab_renders_compactly_and_overview_fits_the_window() {
    creamui_reactive::with_context_scope(|| {
        let state = state();
        let router = router();
        let mut renderer = Renderer::new();
        for (name, theme, width, height) in [
            ("light", Theme::light(), 1180.0, 760.0),
            ("dark", Theme::dark(), 1180.0, 760.0),
            ("compact", Theme::light(), 900.0, 640.0),
            ("small", Theme::dark(), 640.0, 480.0),
            (
                "square",
                Theme::light().with_corners(creamui_theme::CornerStyle::Square),
                900.0,
                640.0,
            ),
            (
                "soft",
                Theme::dark().with_corners(creamui_theme::CornerStyle::Soft),
                1180.0,
                760.0,
            ),
        ] {
            for (index, (path, _)) in PAGES.iter().enumerate() {
                router.navigate(path).unwrap();
                let (_, list) = frame(
                    &mut renderer,
                    &state,
                    &router,
                    theme,
                    Size { width, height },
                );
                assert!(!list.items.is_empty());
                save(&format!("{name}-{index}"), &list);
                if index == 0 && width >= 900.0 {
                    assert!(
                        state.scroll("overview").max_offset() <= 1.0,
                        "overview fits at {width}x{height}: {}",
                        state.scroll("overview").max_offset()
                    );
                }
            }
        }
    });
}
#[test]
fn process_selection_bottom_toolbar_and_confirmation_work() {
    creamui_reactive::with_context_scope(|| {
        let state = state();
        let router = router();
        router.navigate("/apps").unwrap();
        state.task_mode.set(1);
        let mut renderer = Renderer::new();
        let size = Size {
            width: 1180.0,
            height: 760.0,
        };
        let (scene, list) = frame(&mut renderer, &state, &router, Theme::light(), size);
        assert!(state.scroll("processes").max_offset() > 1000.0);
        assert!(list.items.len() < 5000, "only visible rows are mounted");
        scene
            .hit_test(Point { x: 150.0, y: 177.0 })
            .expect("first process row")();
        assert!(state.selected_process.get().is_some());
        let (_, before) = frame(&mut renderer, &state, &router, Theme::light(), size);
        state.scroll("processes").set(450.0);
        let (scene, list) = frame(&mut renderer, &state, &router, Theme::light(), size);
        assert_eq!(
            top_pixels(&before, 158),
            top_pixels(&list, 158),
            "the opaque header is unchanged by scrolling rows"
        );
        save("processes-selected", &list);
        scene
            .hit_test(Point {
                x: 1127.0,
                y: 735.0,
            })
            .expect("End task in fixed bottom toolbar")();
        assert!(matches!(
            state.pending.get(),
            Some(Action::Process(_, ProcessAction::End))
        ));
        assert!(
            state.app.borrow().is_none(),
            "no host action is run by UI tests"
        );
        let (scene, list) = frame(&mut renderer, &state, &router, Theme::light(), size);
        assert!(scene.modal_focus().is_some());
        save("end-task-confirmation", &list);
        state.pending.set(None);
        state.scroll("processes").set(180.0);
        state.accept(fixture());
        frame(&mut renderer, &state, &router, Theme::light(), size);
        assert_eq!(state.scroll("processes").offset(), 180.0);
        state.process_search.set_value("no-such-process");
        frame(&mut renderer, &state, &router, Theme::light(), size);
        assert_eq!(state.scroll("processes").max_offset(), 0.0);
    });
}
#[test]
fn device_selection_and_network_filters_are_independent() {
    creamui_reactive::with_context_scope(|| {
        let state = state();
        let router = router();
        let mut renderer = Renderer::new();
        let size = Size {
            width: 1180.0,
            height: 760.0,
        };
        router.navigate("/performance").unwrap();
        let (scene, _) = frame(&mut renderer, &state, &router, Theme::light(), size);
        scene
            .hit_test(Point { x: 120.0, y: 200.0 })
            .expect("Memory device")();
        assert_eq!(state.resource.get(), Resource::Memory);
        for (name, resource) in [
            ("memory", Resource::Memory),
            ("disk", Resource::Disk("nvme0n1".into())),
            ("gpu", Resource::Gpu("card1".into())),
            ("interface", Resource::Network("enp5s0".into())),
        ] {
            state.resource.set(resource);
            let (_, list) = frame(&mut renderer, &state, &router, Theme::light(), size);
            save(&format!("performance-{name}"), &list);
        }
        router.navigate("/network").unwrap();
        assert_eq!(state.network_mode.get(), 0);
        assert!(state.interface.get().is_none());
        let (scene, _) = frame(&mut renderer, &state, &router, Theme::light(), size);
        scene
            .hit_test(Point { x: 60.0, y: 119.0 })
            .expect("Ports selector")();
        assert_eq!(state.network_mode.get(), 1);
        let (scene, _) = frame(&mut renderer, &state, &router, Theme::light(), size);
        scene
            .hit_test(Point { x: 60.0, y: 241.0 })
            .expect("interface filter")();
        assert_eq!(state.interface.get().as_deref(), Some("enp5s0"));
        let (scene, list) = frame(&mut renderer, &state, &router, Theme::light(), size);
        save("network-ports", &list);
        scene
            .hit_test(Point { x: 60.0, y: 82.0 })
            .expect("Connections selector")();
        assert_eq!(state.network_mode.get(), 0);
        assert_eq!(state.interface.get().as_deref(), Some("enp5s0"));
        let (_, list) = frame(&mut renderer, &state, &router, Theme::light(), size);
        save("network-connections-interface", &list);
        state.interface.set(None);
        let (_, list) = frame(&mut renderer, &state, &router, Theme::light(), size);
        save("network-connections-all", &list);
        state.scroll("network:connections:all").set(180.0);
        let (_, scrolled) = frame(&mut renderer, &state, &router, Theme::light(), size);
        assert_eq!(
            top_pixels(&list, 298),
            top_pixels(&scrolled, 298),
            "traffic graphs and table header remain above the scrolling rows"
        );
        save("network-connections-scrolled", &scrolled);
    });
}
#[test]
fn bound_ports_and_interfaces_filter_correctly() {
    let s = fixture();
    let nic = &s.network[0];
    assert!(network::matches_interface(&s.sockets[0], Some(nic)));
    assert!(!network::matches_interface(&s.sockets[70], Some(nic)));
    assert!(network::matches_interface(&s.sockets[80], Some(nic)));
    assert!(network::is_port(&s.sockets[80]));
    assert!(network::is_port(&s.sockets[81]));
    assert!(!network::is_port(&s.sockets[0]));
}
#[test]
fn apps_expand_and_service_changes_require_an_explicit_action() {
    creamui_reactive::with_context_scope(|| {
        let state = state();
        let router = router();
        let mut renderer = Renderer::new();
        let size = Size {
            width: 1180.0,
            height: 760.0,
        };
        router.navigate("/apps").unwrap();
        let (scene, _) = frame(&mut renderer, &state, &router, Theme::light(), size);
        scene
            .hit_test(Point { x: 1100.0, y: 80.0 })
            .expect("Processes toggle")();
        assert_eq!(state.task_mode.get(), 1);
        let (scene, _) = frame(&mut renderer, &state, &router, Theme::light(), size);
        scene
            .hit_test(Point { x: 995.0, y: 80.0 })
            .expect("Applications toggle")();
        assert_eq!(state.task_mode.get(), 0);
        let (scene, _) = frame(&mut renderer, &state, &router, Theme::light(), size);
        scene
            .hit_test(Point { x: 100.0, y: 177.0 })
            .expect("application group")();
        assert!(state.expanded_app.get().is_some());
        let (_, list) = frame(&mut renderer, &state, &router, Theme::light(), size);
        save("apps-expanded", &list);
        router.navigate("/services").unwrap();
        let (scene, list) = frame(&mut renderer, &state, &router, Theme::light(), size);
        save("services-selected", &list);
        scene
            .hit_test(Point { x: 100.0, y: 207.0 })
            .expect("running service")();
        assert_eq!(
            state.selected_service.get().as_deref(),
            Some("sample-1.service")
        );
        let (scene, _) = frame(&mut renderer, &state, &router, Theme::light(), size);
        scene
            .hit_test(Point {
                x: 1060.0,
                y: 735.0,
            })
            .expect("Stop service")();
        assert!(matches!(
            state.pending.get(),
            Some(Action::Service {
                action: ServiceAction::Stop,
                ..
            })
        ));
        assert!(state.app.borrow().is_none());
        state.pending.set(None);
    });
}
#[test]
fn frame_ownership_and_refresh_keep_header_positions_stable() {
    use creamui_core::WindowDragHandle;
    use creamui_render::{CompositorControls, WindowDecorationMode, WindowDecorations};
    creamui_reactive::with_context_scope(|| {
        creamui_reactive::provide_context(WindowDragHandle::new(|| {}));
        let decorations = Signal::new(WindowDecorations::default());
        creamui_reactive::provide_context(decorations.clone());
        let state = state();
        let router = router();
        let mut renderer = Renderer::new();
        let size = Size {
            width: 1180.0,
            height: 760.0,
        };
        let (before, _) = frame(&mut renderer, &state, &router, Theme::light(), size);
        state.refreshing.set(true);
        let (after, _) = frame(&mut renderer, &state, &router, Theme::light(), size);
        // Refresh is disabled while sampling; tabs and the other controls must
        // retain their hit geometry. Refresh now lives at the leading edge.
        for x in 180..1000 {
            assert_eq!(
                before
                    .hit_test(Point {
                        x: x as f32,
                        y: 26.0
                    })
                    .is_some(),
                after
                    .hit_test(Point {
                        x: x as f32,
                        y: 26.0
                    })
                    .is_some()
            );
        }
        state.refreshing.set(false);
        for mode in [
            WindowDecorationMode::Client,
            WindowDecorationMode::Hybrid,
            WindowDecorationMode::Pending,
        ] {
            decorations.set(WindowDecorations {
                mode,
                controls: CompositorControls {
                    x: 1060,
                    y: 10,
                    width: 112,
                    height: 30,
                },
                ..Default::default()
            });
            let (scene, list) = frame(&mut renderer, &state, &router, Theme::light(), size);
            assert_eq!(
                scene.hit_test(Point { x: 1145.0, y: 26.0 }).is_some(),
                mode == WindowDecorationMode::Client
            );
            assert!(scene.window_drag_at(Point { x: 300.0, y: 49.0 }).is_some());
            if mode == WindowDecorationMode::Client {
                save("client-frame", &list);
            }
        }
    });
}

#[test]
fn shell_material_matches_navigation_and_footer_but_not_content() {
    creamui_reactive::with_context_scope(|| {
        let state = state();
        let router = router();
        let mut renderer = Renderer::new();
        let size = Size {
            width: 1180.0,
            height: 760.0,
        };
        for theme in [Theme::light(), Theme::dark()] {
            for path in ["/performance", "/network", "/apps", "/services"] {
                router.navigate(path).unwrap();
                let (_, list) = frame(&mut renderer, &state, &router, theme, size);
                let mut raster = Rasterizer::new(list.width, list.height);
                raster.render(&list, &Damage::Full);
                let pixel = |x: usize, y: usize| {
                    let offset = (y * list.width as usize + x) * 4;
                    &raster.pixmap().data()[offset..offset + 4]
                };
                assert_eq!(
                    pixel(2, 48),
                    pixel(2, 740),
                    "shell material stays continuous on {path}"
                );
                assert_ne!(
                    pixel(2, 48),
                    pixel(1100, 650),
                    "content has its own material on {path}"
                );
                if matches!(path, "/apps" | "/services") {
                    assert_eq!(pixel(2, 48), pixel(900, 756), "footer matches the titlebar");
                }
                if matches!(path, "/apps" | "/services") {
                    assert_eq!(
                        pixel(2, 48),
                        pixel(500, 80),
                        "section titles and toggles belong to the shell on {path}"
                    );
                }
            }
        }
    });
}

#[test]
fn hybrid_controls_on_either_side_relocate_actions_without_overlap() {
    use creamui_render::{CompositorControls, WindowDecorationMode, WindowDecorations};
    creamui_reactive::with_context_scope(|| {
        let decorations = Signal::new(WindowDecorations::default());
        creamui_reactive::provide_context(decorations.clone());
        let state = state();
        let router = router();
        let mut renderer = Renderer::new();
        for width in [1180.0, 640.0] {
            for left in [false, true] {
                let controls = CompositorControls {
                    x: if left { 8 } else { width as i32 - 120 },
                    y: 10,
                    width: 112,
                    height: 30,
                };
                decorations.set(WindowDecorations {
                    mode: WindowDecorationMode::Hybrid,
                    controls,
                    ..Default::default()
                });
                let (scene, list) = frame(
                    &mut renderer,
                    &state,
                    &router,
                    Theme::light(),
                    Size {
                        width,
                        height: 760.0,
                    },
                );
                for x in controls.x..controls.x + controls.width {
                    assert!(
                        scene
                            .hit_test(Point {
                                x: x as f32,
                                y: 26.0
                            })
                            .is_none(),
                        "reserved compositor controls remain clear"
                    );
                }
                let tools = if width > 950.0 { 160.0 } else { 68.0 };
                let x = if left {
                    width - 8.0 - tools + 16.0
                } else {
                    24.0
                };
                let paused = state.paused.get();
                scene
                    .hit_test(Point { x, y: 26.0 })
                    .expect("Pause remains reachable opposite the window controls")(
                );
                assert_eq!(state.paused.get(), !paused);
                state.paused.set(false);
                save(
                    &format!(
                        "hybrid-{}-{}",
                        if left { "left" } else { "right" },
                        width as u32
                    ),
                    &list,
                );
            }
        }
    });
}

#[test]
fn tabs_stay_centered_in_the_window_for_server_client_and_hybrid_frames() {
    use creamui_render::{CompositorControls, WindowDecorationMode, WindowDecorations};
    creamui_reactive::with_context_scope(|| {
        let decorations = Signal::new(WindowDecorations::default());
        creamui_reactive::provide_context(decorations.clone());
        let state = state();
        let router = router();
        let mut renderer = Renderer::new();
        for width in [640.0, 900.0, 1180.0] {
            for (mode, left) in [
                (WindowDecorationMode::Server, false),
                (WindowDecorationMode::None, false),
                (WindowDecorationMode::Client, false),
                (WindowDecorationMode::Pending, false),
                (WindowDecorationMode::Hybrid, false),
                (WindowDecorationMode::Hybrid, true),
            ] {
                decorations.set(WindowDecorations {
                    mode,
                    controls: CompositorControls {
                        x: if left { 8 } else { width as i32 - 120 },
                        y: 10,
                        width: 112,
                        height: 30,
                    },
                    ..Default::default()
                });
                for busy in [false, true] {
                    state.paused.set(busy);
                    state.refreshing.set(busy);
                    let (scene, list) = frame(
                        &mut renderer,
                        &state,
                        &router,
                        Theme::light(),
                        Size {
                            width,
                            height: 760.0,
                        },
                    );
                    let middle = scene
                        .hit_test(Point {
                            x: width / 2.0,
                            y: 26.0,
                        })
                        .expect("middle navigation tab");
                    let pixels = (0..width as u32)
                        .filter(|x| {
                            scene
                                .hit_test(Point {
                                    x: *x as f32,
                                    y: 26.0,
                                })
                                .is_some_and(|handler| Rc::ptr_eq(handler, middle))
                        })
                        .collect::<Vec<_>>();
                    let center = (*pixels.first().unwrap() + *pixels.last().unwrap()) as f32 / 2.0;
                    assert!((center - width / 2.0).abs() <= 1.0,
                        "middle tab must align with the window center: {mode:?}, left={left}, width={width}, actual={center}");
                    middle();
                    assert_eq!(
                        router.location().pathname,
                        "/performance",
                        "the middle of five tabs is Performance"
                    );
                    router.navigate("/").unwrap();
                    if mode == WindowDecorationMode::Server && !busy {
                        save(&format!("server-centered-{}", width as u32), &list);
                    }
                }
            }
        }
    });
}

#[test]
fn initial_size_is_compact_and_respects_logical_monitor_dimensions() {
    let reference = (1024, 640);
    assert_eq!(initial_window_size(None), reference);
    assert_eq!(initial_window_size(Some((1920, 1080))), reference);
    assert_eq!(initial_window_size(Some((1366, 768))), reference);
    assert_eq!(initial_window_size(Some((0, 1080))), reference);
    for monitor in [(1280, 720), (1024, 600), (800, 600), (720, 1280)] {
        let (width, height) = initial_window_size(Some(monitor));
        assert!(width <= reference.0 && height <= reference.1);
        assert!(width as f64 <= monitor.0 as f64 * 0.85);
        assert!(height as f64 <= monitor.1 as f64 * 0.85);
        assert_eq!((width % 8, height % 8), (0, 0));
        assert!(
            (width as f64 / height as f64 - 1.6).abs() < 0.04,
            "scaling preserves 16:10 within grid rounding"
        );
    }
}

#[test]
fn cpu_selector_shares_the_title_row_and_sixteen_cores_fit_the_compact_default() {
    creamui_reactive::with_context_scope(|| {
        let state = state();
        state.snapshot.update(|s| {
            s.cpu.cores = 16;
            s.cpu.model = "AMD Ryzen 7 5825U with Radeon Graphics".into();
            s.cpu.per_core = vec![2.0; 16];
        });
        state.histories.update(|histories| {
            let samples = histories["cpu:0"].clone();
            for index in 4..16 {
                histories.insert(format!("cpu:{index}"), samples.clone());
            }
        });
        state.per_core.set(true);
        let router = router();
        router.navigate("/performance").unwrap();
        let mut renderer = Renderer::new();
        for (width, height) in [
            initial_window_size(None),
            initial_window_size(Some((1280, 720))),
            (960, 600),
        ] {
            let size = Size {
                width: width as f32,
                height: height as f32,
            };
            let (scene, list) = frame(&mut renderer, &state, &router, Theme::dark(), size);
            assert!(
                state.scroll("performance:cpu").max_offset() <= 1.0,
                "16 cores and technical stats fit the compact window: {}",
                state.scroll("performance:cpu").max_offset()
            );
            save(&format!("performance-compact-16-{width}x{height}"), &list);
            scene
                .hit_test(Point {
                    x: width as f32 - 137.0,
                    y: 80.0,
                })
                .expect("Total switch at the right of the title")();
            assert!(!state.per_core.get());
            let (scene, _) = frame(&mut renderer, &state, &router, Theme::dark(), size);
            scene
                .hit_test(Point {
                    x: width as f32 - 70.0,
                    y: 80.0,
                })
                .expect("Per core switch at the right of the title")();
            assert!(state.per_core.get());
        }
        router.navigate("/apps").unwrap();
        let (width, height) = initial_window_size(None);
        let (_, list) = frame(
            &mut renderer,
            &state,
            &router,
            Theme::dark(),
            Size {
                width: width as f32,
                height: height as f32,
            },
        );
        save("apps-compact-shell-header", &list);
        router.navigate("/services").unwrap();
        let (scene, list) = frame(
            &mut renderer,
            &state,
            &router,
            Theme::dark(),
            Size {
                width: width as f32,
                height: height as f32,
            },
        );
        save("services-compact-shell-header", &list);
        scene
            .hit_test(Point {
                x: width as f32 - 64.0,
                y: 80.0,
            })
            .expect("User scope in the shell header")();
        assert!(state.user_services.get());
        let (scene, _) = frame(
            &mut renderer,
            &state,
            &router,
            Theme::dark(),
            Size {
                width: width as f32,
                height: height as f32,
            },
        );
        scene
            .hit_test(Point {
                x: width as f32 - 140.0,
                y: 80.0,
            })
            .expect("System scope in the shell header")();
        assert!(!state.user_services.get());
    });
}
