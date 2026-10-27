use coconut_background::{gpu, render};
use coconut_core::{ipc, BackgroundConfig, BuiltinShader, ShaderConfig, WallpaperSource};
use image::RgbaImage;
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_layer, delegate_output, delegate_registry, delegate_shm,
    output::{OutputHandler, OutputState},
    reexports::{
        calloop::{channel, EventLoop},
        calloop_wayland_source::WaylandSource,
    },
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    shell::{
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
        WaylandSurface,
    },
    shm::{slot::SlotPool, Shm, ShmHandler},
};
use std::time::{Duration, Instant};
use wayland_client::{
    backend::Backend,
    globals::registry_queue_init,
    protocol::{wl_output, wl_region, wl_shm, wl_surface},
    Connection, Dispatch, Proxy, QueueHandle,
};

const MAX_FRAME_BYTES: usize = 512 * 1024 * 1024;

fn main() {
    if let Err(error) = run() {
        eprintln!("coconut-bg: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let connection = Connection::connect_to_env()
        .map_err(|error| format!("could not connect to Wayland: {error}"))?;
    let (globals, event_queue) = registry_queue_init(&connection)
        .map_err(|error| format!("could not enumerate Wayland globals: {error}"))?;
    let queue_handle = event_queue.handle();
    let mut event_loop: EventLoop<BackgroundApp> =
        EventLoop::try_new().map_err(|error| format!("could not create event loop: {error}"))?;
    let loop_handle = event_loop.handle();
    WaylandSource::new(connection.clone(), event_queue)
        .insert(loop_handle.clone())
        .map_err(|error| format!("could not register Wayland event source: {error}"))?;

    let compositor = CompositorState::bind(&globals, &queue_handle)
        .map_err(|error| format!("wl_compositor is unavailable: {error}"))?;
    let layer_shell = LayerShell::bind(&globals, &queue_handle)
        .map_err(|error| format!("the compositor does not support layer-shell: {error}"))?;
    let shm = Shm::bind(&globals, &queue_handle)
        .map_err(|error| format!("wl_shm is unavailable: {error}"))?;
    let pool =
        SlotPool::new(4, &shm).map_err(|error| format!("could not allocate SHM pool: {error}"))?;

    let config = BackgroundConfig::load();
    let image = load_image(&config);
    let mut app = BackgroundApp {
        registry_state: RegistryState::new(&globals),
        output_state: OutputState::new(&globals, &queue_handle),
        compositor,
        layer_shell,
        shm,
        pool,
        config,
        image,
        surfaces: Vec::new(),
        backend: connection.backend(),
        queue: queue_handle.clone(),
        start: Instant::now(),
        gpu: None,
        gpu_failed: false,
    };

    let outputs: Vec<_> = app.output_state.outputs().collect();
    for output in outputs {
        app.add_output(&queue_handle, output);
    }

    // Settings emits a D-Bus signal after atomically saving bg.toml. Bridge
    // that receiver into the Wayland event loop so redraws happen immediately
    // without polling or a continuously-running frame callback.
    let (config_sender, config_receiver) = channel::channel();
    std::thread::spawn(move || {
        for config in ipc::listen_for_background_config() {
            if config_sender.send(config).is_err() {
                return;
            }
        }
    });
    loop_handle
        .insert_source(config_receiver, |event, _, app| {
            if let channel::Event::Msg(config) = event {
                app.set_config(config);
            }
        })
        .map_err(|error| format!("could not register configuration listener: {error}"))?;

    loop {
        event_loop
            .dispatch(None, &mut app)
            .map_err(|error| format!("Wayland event loop stopped: {error}"))?;
    }
}

struct BackgroundSurface {
    // Held directly rather than behind the layer so it drops first: the wgpu
    // surface borrows the raw `wl_surface` pointer, which must still be alive.
    gpu: Option<gpu::GpuSurface>,
    output: wl_output::WlOutput,
    layer: LayerSurface,
    logical_size: (u32, u32),
    scale: i32,
    configured: bool,
    redraw: bool,
    frame_pending: bool,
    /// When the last shader frame was presented, for target-FPS pacing.
    last_present: Option<Instant>,
}

struct BackgroundApp {
    registry_state: RegistryState,
    output_state: OutputState,
    compositor: CompositorState,
    layer_shell: LayerShell,
    // Kept alive for the SlotPool and delegated wl_shm events.
    shm: Shm,
    pool: SlotPool,
    config: BackgroundConfig,
    image: Option<RgbaImage>,
    surfaces: Vec<BackgroundSurface>,
    backend: Backend,
    queue: QueueHandle<BackgroundApp>,
    start: Instant,
    gpu: Option<gpu::Gpu>,
    gpu_failed: bool,
}

impl BackgroundApp {
    fn add_output(&mut self, queue: &QueueHandle<Self>, output: wl_output::WlOutput) {
        if self.surfaces.iter().any(|surface| surface.output == output) {
            return;
        }

        let surface = self.compositor.create_surface(queue);
        let empty_input = self.compositor.wl_compositor().create_region(queue, ());
        empty_input.add(0, 0, 0, 0);
        surface.set_input_region(Some(&empty_input));
        empty_input.destroy();

        let layer = self.layer_shell.create_layer_surface(
            queue,
            surface,
            Layer::Bottom,
            Some("desktop"),
            Some(&output),
        );
        layer.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
        layer.set_size(0, 0);
        layer.set_exclusive_zone(-1);
        layer.set_keyboard_interactivity(KeyboardInteractivity::None);
        layer.commit();
        self.surfaces.push(BackgroundSurface {
            gpu: None,
            output,
            layer,
            logical_size: (0, 0),
            scale: 1,
            configured: false,
            redraw: true,
            frame_pending: false,
            last_present: None,
        });
    }

    fn set_config(&mut self, config: BackgroundConfig) {
        self.image = load_image(&config);
        self.config = config;
        for surface in &mut self.surfaces {
            surface.redraw = true;
        }
        self.draw();
    }

    /// Lazily creates the shared GPU handles on first use; `false` means no
    /// usable adapter exists and shaders fall back to the solid color.
    fn ensure_gpu(&mut self) -> bool {
        if self.gpu.is_none() && !self.gpu_failed {
            match gpu::Gpu::new() {
                Ok(gpu) => self.gpu = Some(gpu),
                Err(error) => {
                    eprintln!("coconut-bg: {error}; shader wallpapers are unavailable");
                    self.gpu_failed = true;
                }
            }
        }
        self.gpu.is_some()
    }

    fn draw(&mut self) {
        let config = self.config.clone();
        for index in 0..self.surfaces.len() {
            let surface = &self.surfaces[index];
            if !surface.configured || !surface.redraw {
                continue;
            }
            if config.source == WallpaperSource::Shader {
                let shader_config = config.shader_config(config.shader);
                self.draw_shader(index, config.shader, &shader_config);
            } else {
                self.draw_frame(index, &config);
            }
        }
    }

    /// Paints one CPU-rendered frame into a `wl_shm` buffer. Also the fallback
    /// when [`WallpaperSource::Shader`] has no working GPU.
    fn draw_frame(&mut self, index: usize, config: &BackgroundConfig) {
        let surface = &mut self.surfaces[index];
        let Some((width, height)) = physical_size(surface.logical_size, surface.scale) else {
            eprintln!("coconut-bg: ignoring an output with an invalid or oversized size");
            surface.redraw = false;
            return;
        };
        let image = self.image.as_ref();
        let frame = render::render(config, image, width, height);
        let stride = i32::try_from(width)
            .ok()
            .and_then(|width| width.checked_mul(4));
        let Some(stride) = stride else {
            eprintln!("coconut-bg: output stride is too large");
            surface.redraw = false;
            return;
        };
        let Ok((buffer, canvas)) = self.pool.create_buffer(
            width as i32,
            height as i32,
            stride,
            wl_shm::Format::Argb8888,
        ) else {
            eprintln!("coconut-bg: could not allocate an output buffer");
            surface.redraw = false;
            return;
        };
        for (source, destination) in frame.pixels().zip(canvas.chunks_exact_mut(4)) {
            // wl_shm ARGB8888 is stored as BGRA on little-endian hosts.
            destination.copy_from_slice(&[source[2], source[1], source[0], source[3]]);
        }
        let surface = &mut self.surfaces[index];
        surface.layer.wl_surface().set_buffer_scale(surface.scale);
        surface
            .layer
            .wl_surface()
            .damage_buffer(0, 0, width as i32, height as i32);
        if let Err(error) = buffer.attach_to(surface.layer.wl_surface()) {
            eprintln!("coconut-bg: could not attach output buffer: {error}");
            return;
        }
        surface.layer.commit();
        surface.redraw = false;
    }

    /// Presents one animated frame through the output's GPU swapchain and arms
    /// the frame callback that drives the next one.
    fn draw_shader(&mut self, index: usize, shader: BuiltinShader, shader_config: &ShaderConfig) {
        let (wl_surface, width, height, scale) = {
            let surface = &mut self.surfaces[index];
            let Some((width, height)) = physical_size(surface.logical_size, surface.scale) else {
                eprintln!("coconut-bg: ignoring an output with an invalid or oversized size");
                surface.redraw = false;
                return;
            };
            (
                surface.layer.wl_surface().clone(),
                width,
                height,
                surface.scale,
            )
        };

        let surface_ptr = wl_surface.id().as_ptr() as *mut std::ffi::c_void;
        let display_ptr = self.backend.display_ptr() as *mut std::ffi::c_void;
        if self.gpu_failed || !self.ensure_gpu() {
            self.draw_frame(index, &self.config.clone());
            return;
        }
        let gpu = self.gpu.as_ref().expect("gpu checked above");

        let surface = &mut self.surfaces[index];
        let state = match &mut surface.gpu {
            Some(state) => state,
            None => {
                // SAFETY: `display_ptr`/`surface_ptr` point at the live
                // `wl_display`/`wl_surface`; the layer outlives `GpuSurface`
                // because it is dropped first (field order).
                let created = unsafe {
                    gpu::GpuSurface::new(gpu, display_ptr, surface_ptr, shader, width, height)
                };
                match created {
                    Ok(state) => surface.gpu.insert(state),
                    Err(error) => {
                        eprintln!("coconut-bg: {error}; falling back to the solid color");
                        self.gpu_failed = true;
                        self.draw_frame(index, &self.config.clone());
                        return;
                    }
                }
            }
        };
        if state.shader() != shader {
            if let Err(error) = state.set_shader(&gpu.device, shader) {
                eprintln!("coconut-bg: {error}");
            }
        }
        state.resize(&gpu.device, width, height);
        wl_surface.set_buffer_scale(scale);
        // Frame callbacks bind to the next wl_surface.commit, which happens
        // inside render(). Arming afterwards would leave the callback waiting
        // for a commit that never arrives.
        if !surface.frame_pending {
            wl_surface.frame(&self.queue, wl_surface.clone());
            surface.frame_pending = true;
        }
        let time = self.start.elapsed().as_secs_f32() * shader_config.speed;
        if state.render(gpu, time, shader_config) {
            surface.last_present = Some(Instant::now());
        } else {
            // No frame was presented, so the armed callback never fired: bind
            // it to an empty commit so acquisition is retried at the next
            // repaint instead of stalling the animation.
            wl_surface.commit();
        }
        surface.redraw = false;
    }
}

fn load_image(config: &BackgroundConfig) -> Option<RgbaImage> {
    let path = config.wallpaper.as_ref()?.clone();
    if !path.is_file() {
        eprintln!("coconut-bg: wallpaper does not exist: {}", path.display());
        return None;
    }
    match image::open(&path) {
        Ok(image) => Some(image.to_rgba8()),
        Err(error) => {
            eprintln!("coconut-bg: could not decode {}: {error}", path.display());
            None
        }
    }
}

fn physical_size(logical: (u32, u32), scale: i32) -> Option<(u32, u32)> {
    let scale = u32::try_from(scale.max(1)).ok()?;
    let width = logical.0.checked_mul(scale)?;
    let height = logical.1.checked_mul(scale)?;
    let bytes = (width as usize)
        .checked_mul(height as usize)?
        .checked_mul(4)?;
    (width > 0 && height > 0 && bytes <= MAX_FRAME_BYTES).then_some((width, height))
}

impl CompositorHandler for BackgroundApp {
    fn scale_factor_changed(
        &mut self,
        _connection: &Connection,
        _queue: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        scale: i32,
    ) {
        if let Some(background) = self
            .surfaces
            .iter_mut()
            .find(|background| background.layer.wl_surface() == surface)
        {
            background.scale = scale.max(1);
            background.redraw = true;
        }
        self.draw();
    }

    fn transform_changed(
        &mut self,
        _connection: &Connection,
        _queue: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _transform: wl_output::Transform,
    ) {
    }

    /// Each armed callback keeps the animation loop alive: present a frame,
    /// then arm the next. The compositor stops repainting hidden or idle
    /// surfaces, which pauses the loop for free.
    fn frame(
        &mut self,
        _connection: &Connection,
        _queue: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
        let Some(index) = self
            .surfaces
            .iter()
            .position(|background| background.layer.wl_surface() == surface)
        else {
            return;
        };
        self.surfaces[index].frame_pending = false;
        if self.config.source != WallpaperSource::Shader || !self.surfaces[index].configured {
            return;
        }
        let shader = self.config.shader;
        let shader_config = self.config.shader_config(shader);
        let frame_interval =
            Duration::from_secs_f64(1.0 / f64::from(shader_config.fps.clamp(1, 60)));
        let now = Instant::now();
        if let Some(last) = self.surfaces[index].last_present {
            if now.duration_since(last) < frame_interval {
                // Too soon for the target frame rate: an empty commit keeps the
                // callback chain alive at the display's pace until the
                // interval has elapsed.
                surface.frame(&self.queue, surface.clone());
                surface.commit();
                self.surfaces[index].frame_pending = true;
                return;
            }
        }
        self.draw_shader(index, shader, &shader_config);
    }

    fn surface_enter(
        &mut self,
        _connection: &Connection,
        _queue: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _connection: &Connection,
        _queue: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }
}

impl LayerShellHandler for BackgroundApp {
    fn closed(
        &mut self,
        _connection: &Connection,
        _queue: &QueueHandle<Self>,
        layer: &LayerSurface,
    ) {
        self.surfaces.retain(|surface| surface.layer != *layer);
    }

    fn configure(
        &mut self,
        _connection: &Connection,
        _queue: &QueueHandle<Self>,
        layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        if let Some(surface) = self
            .surfaces
            .iter_mut()
            .find(|surface| surface.layer == *layer)
        {
            surface.logical_size = configure.new_size;
            surface.configured = configure.new_size.0 > 0 && configure.new_size.1 > 0;
            surface.redraw = true;
        }
        self.draw();
    }
}

impl OutputHandler for BackgroundApp {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _connection: &Connection,
        queue: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        self.add_output(queue, output);
    }

    fn update_output(
        &mut self,
        _connection: &Connection,
        _queue: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        if let Some(surface) = self
            .surfaces
            .iter_mut()
            .find(|surface| surface.output == output)
        {
            surface.redraw = true;
        }
        self.draw();
    }

    fn output_destroyed(
        &mut self,
        _connection: &Connection,
        _queue: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        self.surfaces.retain(|surface| surface.output != output);
    }
}

impl ShmHandler for BackgroundApp {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl Dispatch<wl_region::WlRegion, ()> for BackgroundApp {
    fn event(
        _: &mut Self,
        _: &wl_region::WlRegion,
        _: wl_region::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

delegate_compositor!(BackgroundApp);
delegate_output!(BackgroundApp);
delegate_shm!(BackgroundApp);
delegate_layer!(BackgroundApp);
delegate_registry!(BackgroundApp);

impl ProvidesRegistryState for BackgroundApp {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }

    registry_handlers![OutputState];
}
