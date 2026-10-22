//! GPU plumbing for animated shader backgrounds: a shared `wgpu` device and,
//! per output, a swapchain built directly on the layer-shell `wl_surface`.
//! Everything here is fallible — a machine without a usable adapter keeps the
//! CPU-rendered fallback.

use crate::shaders;
use coconut_core::{BuiltinShader, DesktopColor, ShaderConfig, MAX_SHADER_COLORS};
use raw_window_handle::{
    RawDisplayHandle, RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle,
};
use std::ffi::c_void;
use std::ptr::NonNull;

/// Process-wide GPU state shared by every output surface.
pub struct Gpu {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl Gpu {
    pub fn new() -> Result<Self, String> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY | wgpu::Backends::GL,
            ..Default::default()
        });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))
        .ok_or_else(|| "no compatible GPU adapter was found".to_owned())?;
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default(), None))
                .map_err(|error| format!("could not create a GPU device: {error}"))?;
        Ok(Self {
            instance,
            adapter,
            device,
            queue,
        })
    }
}

/// The render pipeline and uniform plumbing for one [`BuiltinShader`] and one
/// target format. Split from [`GpuSurface`] so offscreen targets (previews,
/// tests) reuse the same drawing path.
pub struct ShaderRenderer {
    pipeline: wgpu::RenderPipeline,
    globals: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    shader: BuiltinShader,
}

impl ShaderRenderer {
    pub fn new(
        device: &wgpu::Device,
        shader: BuiltinShader,
        format: wgpu::TextureFormat,
    ) -> Result<Self, String> {
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(shader.name()),
            source: wgpu::ShaderSource::Wgsl(shaders::wgsl(shader).into()),
        });
        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("shader globals"),
            size: 16 + (MAX_SHADER_COLORS * 16) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("shader globals"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shader globals"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals.as_entire_binding(),
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(shader.name()),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(shader.name()),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: "vs_main",
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        if let Some(error) = pollster::block_on(device.pop_error_scope()) {
            return Err(format!(
                "could not build the {name} pipeline: {error}",
                name = shader.name()
            ));
        }
        Ok(Self {
            pipeline,
            globals,
            bind_group,
            shader,
        })
    }

    pub fn shader(&self) -> BuiltinShader {
        self.shader
    }

    /// Draws one frame into `view`. The caller owns the encoder so on-screen
    /// and offscreen targets submit identically. `time` is the shader clock
    /// (already speed-scaled) and `colors` the configured palette; missing
    /// slots fall back to this shader's defaults.
    pub fn draw(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        width: u32,
        height: u32,
        time: f32,
        colors: &[DesktopColor],
    ) {
        let mut bytes = [0u8; 16 + MAX_SHADER_COLORS * 16];
        let uniforms = [
            width as f32,
            height as f32,
            time,
            width as f32 / height.max(1) as f32,
        ];
        for (chunk, value) in bytes[..16].chunks_exact_mut(4).zip(uniforms) {
            chunk.copy_from_slice(&value.to_ne_bytes());
        }
        for (slot, chunk) in bytes[16..].chunks_exact_mut(16).enumerate() {
            let color = colors.get(slot).copied().unwrap_or_else(|| {
                self.shader
                    .palette()
                    .get(slot)
                    .map(|(_, default)| *default)
                    .unwrap_or_default()
            });
            let f32s = [
                f32::from(color.r) / 255.0,
                f32::from(color.g) / 255.0,
                f32::from(color.b) / 255.0,
                1.0,
            ];
            for (field, value) in chunk.chunks_exact_mut(4).zip(f32s) {
                field.copy_from_slice(&value.to_ne_bytes());
            }
        }
        queue.write_buffer(&self.globals, 0, &bytes);
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(self.shader.name()),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

/// One output's wgpu swapchain, attached straight to the layer-shell
/// `wl_surface` so presented frames never bounce through CPU memory.
pub struct GpuSurface {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    renderer: ShaderRenderer,
}

impl GpuSurface {
    /// `display` and `surface` are the raw `wl_display`/`wl_surface` pointers;
    /// both must stay valid for as long as the returned value lives.
    pub unsafe fn new(
        gpu: &Gpu,
        display: *mut c_void,
        surface: *mut c_void,
        shader: BuiltinShader,
        width: u32,
        height: u32,
    ) -> Result<Self, String> {
        let display = NonNull::new(display).ok_or("the wl_display pointer is null")?;
        let surface_ptr = NonNull::new(surface).ok_or("the wl_surface pointer is null")?;
        let target = wgpu::SurfaceTargetUnsafe::RawHandle {
            raw_display_handle: RawDisplayHandle::Wayland(WaylandDisplayHandle::new(display)),
            raw_window_handle: RawWindowHandle::Wayland(WaylandWindowHandle::new(surface_ptr)),
        };
        let surface = unsafe { gpu.instance.create_surface_unsafe(target) }
            .map_err(|error| format!("could not create a GPU surface: {error}"))?;
        let capabilities = surface.get_capabilities(&gpu.adapter);
        let format = capabilities
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .or_else(|| capabilities.formats.first().copied())
            .ok_or("the GPU surface offers no texture formats")?;
        let alpha_mode = capabilities
            .alpha_modes
            .first()
            .copied()
            .unwrap_or(wgpu::CompositeAlphaMode::Auto);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode,
            view_formats: Vec::new(),
        };
        surface.configure(&gpu.device, &config);
        let renderer = ShaderRenderer::new(&gpu.device, shader, format)?;
        Ok(Self {
            surface,
            config,
            renderer,
        })
    }

    pub fn shader(&self) -> BuiltinShader {
        self.renderer.shader()
    }

    pub fn set_shader(
        &mut self,
        device: &wgpu::Device,
        shader: BuiltinShader,
    ) -> Result<(), String> {
        self.renderer = ShaderRenderer::new(device, shader, self.config.format)?;
        Ok(())
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if width == self.config.width && height == self.config.height {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(device, &self.config);
    }

    /// Renders one animation frame and presents it, committing the
    /// `wl_surface` through the swapchain. Returns `false` when nothing was
    /// presented — the armed frame callback then binds to the next commit.
    pub fn render(&mut self, gpu: &Gpu, time: f32, config: &ShaderConfig) -> bool {
        let frame = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(wgpu::SurfaceError::Outdated | wgpu::SurfaceError::Lost) => {
                self.surface.configure(&gpu.device, &self.config);
                match self.surface.get_current_texture() {
                    Ok(frame) => frame,
                    Err(error) => {
                        eprintln!("coconut-bg: could not acquire a GPU frame: {error}");
                        return false;
                    }
                }
            }
            Err(wgpu::SurfaceError::Timeout) => return false,
            Err(error) => {
                eprintln!("coconut-bg: could not acquire a GPU frame: {error}");
                return false;
            }
        };
        let view = frame.texture.create_view(&Default::default());
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("background"),
            });
        self.renderer.draw(
            &gpu.queue,
            &mut encoder,
            &view,
            self.config.width,
            self.config.height,
            time,
            &config.colors,
        );
        gpu.queue.submit([encoder.finish()]);
        frame.present();
        true
    }
}
