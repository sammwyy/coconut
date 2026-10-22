//! Renders a built-in shader into a PNG without a compositor:
//! `cargo run -p coconut-bg --example shader_preview -- swirl out.png [time]`

use coconut_bg::gpu::{Gpu, ShaderRenderer};
use coconut_core::BuiltinShader;

fn main() {
    let mut args = std::env::args().skip(1);
    let name = args.next().unwrap_or_else(|| "swirl".to_owned());
    let output = args.next().unwrap_or_else(|| format!("{name}.png"));
    let time: f32 = args.next().and_then(|arg| arg.parse().ok()).unwrap_or(12.0);
    let shader = *BuiltinShader::ALL
        .iter()
        .find(|shader| shader.name().eq_ignore_ascii_case(&name))
        .unwrap_or_else(|| {
            eprintln!("unknown shader {name:?}");
            std::process::exit(2);
        });

    let (width, height) = (960, 540);
    let gpu = Gpu::new().expect("GPU init");
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("preview"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let renderer =
        ShaderRenderer::new(&gpu.device, shader, texture.format()).expect("pipeline for preview");
    let view = texture.create_view(&Default::default());
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    renderer.draw(&gpu.queue, &mut encoder, &view, width, height, time, &[]);

    let row_bytes = width * 4;
    let stride =
        row_bytes.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let staging = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: u64::from(stride) * u64::from(height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::ImageCopyBuffer {
            buffer: &staging,
            layout: wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(stride),
                rows_per_image: None,
            },
        },
        texture.size(),
    );
    gpu.queue.submit([encoder.finish()]);

    let slice = staging.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    gpu.device.poll(wgpu::Maintain::Wait);
    let data = slice.get_mapped_range();
    let mut rgba = Vec::with_capacity((row_bytes * height) as usize);
    for row in data.chunks(stride as usize) {
        rgba.extend_from_slice(&row[..row_bytes as usize]);
    }
    image::save_buffer(
        &output,
        &rgba,
        width,
        height,
        image::ExtendedColorType::Rgba8,
    )
    .expect("write preview png");
    println!("wrote {output}");
}
