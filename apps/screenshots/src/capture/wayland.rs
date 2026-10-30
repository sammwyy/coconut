//! `wlr-screencopy-unstable-v1` capture: every advertised output is copied
//! into a shared-memory buffer, then composited at native pixels using the
//! logical layout reported by `xdg-output`.

use crate::image::Shot;
use smithay_client_toolkit::{error::GlobalError, globals::ProvidesBoundGlobal, shm::raw::RawPool};
use wayland_client::{
    protocol::{
        wl_buffer::WlBuffer,
        wl_output::WlOutput,
        wl_registry::WlRegistry,
        wl_shm::{self, WlShm},
    },
    Connection, Dispatch, EventQueue, QueueHandle, WEnum,
};
use wayland_protocols::xdg::xdg_output::zv1::client::{
    zxdg_output_manager_v1::ZxdgOutputManagerV1,
    zxdg_output_v1::{self, ZxdgOutputV1},
};
use wayland_protocols_wlr::screencopy::v1::client::{
    zwlr_screencopy_frame_v1::{self, ZwlrScreencopyFrameV1},
    zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1,
};

#[derive(Default)]
struct LogicalGeometry {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    done: bool,
}

#[derive(Default)]
struct Frame {
    shm: Option<(wl_shm::Format, u32, u32, u32)>,
    pool: Option<RawPool>,
    buffer: Option<WlBuffer>,
    ready: bool,
    failed: bool,
}

#[derive(Default)]
struct State {
    shm: Option<WlShm>,
    screencopy: Option<ZwlrScreencopyManagerV1>,
    xdg_output: Option<ZxdgOutputManagerV1>,
    outputs: Vec<WlOutput>,
    geometry: Vec<LogicalGeometry>,
    frames: Vec<Frame>,
}

struct BoundShm(WlShm);

impl ProvidesBoundGlobal<WlShm, 1> for BoundShm {
    fn bound_global(&self) -> Result<WlShm, GlobalError> {
        Ok(self.0.clone())
    }
}

impl Dispatch<WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &WlRegistry,
        event: wayland_client::protocol::wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        use wayland_client::protocol::wl_registry::Event;
        let Event::Global {
            name,
            interface,
            version,
        } = event
        else {
            return;
        };
        match interface.as_str() {
            "wl_shm" => {
                state.shm = Some(registry.bind::<WlShm, _, _>(name, 1, qh, ()));
            }
            "zwlr_screencopy_manager_v1" => {
                state.screencopy = Some(registry.bind::<ZwlrScreencopyManagerV1, _, _>(
                    name,
                    version.min(3),
                    qh,
                    (),
                ));
            }
            "zxdg_output_manager_v1" => {
                state.xdg_output =
                    Some(registry.bind::<ZxdgOutputManagerV1, _, _>(name, version.min(3), qh, ()));
            }
            "wl_output" => {
                state
                    .outputs
                    .push(registry.bind::<WlOutput, _, _>(name, version.min(4), qh, ()));
            }
            _ => {}
        }
    }
}

impl Dispatch<WlOutput, ()> for State {
    fn event(
        _: &mut Self,
        _: &WlOutput,
        _: wayland_client::protocol::wl_output::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlShm, ()> for State {
    fn event(
        _: &mut Self,
        _: &WlShm,
        _: wl_shm::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZxdgOutputManagerV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &ZxdgOutputManagerV1,
        _: <ZxdgOutputManagerV1 as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZxdgOutputV1, usize> for State {
    fn event(
        state: &mut Self,
        _: &ZxdgOutputV1,
        event: zxdg_output_v1::Event,
        &index: &usize,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let Some(geometry) = state.geometry.get_mut(index) else {
            return;
        };
        match event {
            zxdg_output_v1::Event::LogicalPosition { x, y } => {
                geometry.x = x;
                geometry.y = y;
            }
            zxdg_output_v1::Event::LogicalSize { width, height } => {
                geometry.width = width;
                geometry.height = height;
            }
            zxdg_output_v1::Event::Done => geometry.done = true,
            _ => {}
        }
    }
}

impl Dispatch<ZwlrScreencopyManagerV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &ZwlrScreencopyManagerV1,
        _: <ZwlrScreencopyManagerV1 as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwlrScreencopyFrameV1, usize> for State {
    fn event(
        state: &mut Self,
        frame: &ZwlrScreencopyFrameV1,
        event: zwlr_screencopy_frame_v1::Event,
        &index: &usize,
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        let Some(slot) = state.frames.get_mut(index) else {
            return;
        };
        match event {
            zwlr_screencopy_frame_v1::Event::Buffer {
                format: WEnum::Value(format),
                width,
                height,
                stride,
            } => {
                slot.shm = Some((format, width, height, stride));
            }
            zwlr_screencopy_frame_v1::Event::BufferDone => {
                let Some((format, width, height, stride)) = slot.shm else {
                    slot.failed = true;
                    return;
                };
                let Some(shm) = state.shm.clone() else {
                    slot.failed = true;
                    return;
                };
                let len = stride as usize * height as usize;
                let mut pool = match RawPool::new(len.max(1), &BoundShm(shm)) {
                    Ok(pool) => pool,
                    Err(error) => {
                        eprintln!("coconut-screenshots: shm pool failed: {error}");
                        slot.failed = true;
                        return;
                    }
                };
                let buffer = pool.create_buffer(
                    0,
                    width as i32,
                    height as i32,
                    stride as i32,
                    format,
                    (),
                    qh,
                );
                slot.pool = Some(pool);
                slot.buffer = Some(buffer.clone());
                frame.copy(&buffer);
            }
            zwlr_screencopy_frame_v1::Event::Ready { .. } => slot.ready = true,
            zwlr_screencopy_frame_v1::Event::Failed => slot.failed = true,
            _ => {}
        }
    }
}

impl Dispatch<WlBuffer, ()> for State {
    fn event(
        _: &mut Self,
        _: &WlBuffer,
        _: wayland_client::protocol::wl_buffer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

fn roundtrip(queue: &mut EventQueue<State>, state: &mut State) -> Result<(), String> {
    queue
        .roundtrip(state)
        .map(|_| ())
        .map_err(|error| format!("Wayland roundtrip failed: {error}"))
}

/// Reads one captured frame into a `Shot`, converting the shm format to
/// premultiplied RGBA8.
fn frame_pixels(frame: &mut Frame) -> Result<Shot, String> {
    let Some((format, width, height, stride)) = frame.shm else {
        return Err("compositor never described the capture buffer".into());
    };
    let Some(pool) = frame.pool.as_mut() else {
        return Err("compositor never copied the capture buffer".into());
    };
    let mapping = pool.mmap();
    let mut pixels = vec![0u8; width as usize * height as usize * 4];
    for y in 0..height as usize {
        let row = y * stride as usize;
        for x in 0..width as usize {
            let from = row + x * 4;
            let to = (y * width as usize + x) * 4;
            let (b0, b1, b2, b3) = (
                mapping[from],
                mapping[from + 1],
                mapping[from + 2],
                mapping[from + 3],
            );
            let (r, g, b, a) = match format {
                wl_shm::Format::Argb8888 => (b2, b1, b0, b3),
                wl_shm::Format::Xrgb8888 => (b2, b1, b0, 255),
                wl_shm::Format::Abgr8888 => (b0, b1, b2, b3),
                wl_shm::Format::Xbgr8888 => (b0, b1, b2, 255),
                wl_shm::Format::Rgba8888 => (b0, b1, b2, b3),
                wl_shm::Format::Rgbx8888 => (b0, b1, b2, 255),
                wl_shm::Format::Bgra8888 => (b2, b1, b0, b3),
                wl_shm::Format::Bgrx8888 => (b2, b1, b0, 255),
                other => return Err(format!("unsupported shm format {other:?}")),
            };
            let alpha = a as u16;
            pixels[to] = ((r as u16 * alpha + 127) / 255) as u8;
            pixels[to + 1] = ((g as u16 * alpha + 127) / 255) as u8;
            pixels[to + 2] = ((b as u16 * alpha + 127) / 255) as u8;
            pixels[to + 3] = a;
        }
    }
    Shot::new(width, height, pixels)
}

pub fn screen() -> Result<Shot, String> {
    let connection =
        Connection::connect_to_env().map_err(|error| format!("Wayland connect failed: {error}"))?;
    let display = connection.display();
    let mut queue: EventQueue<State> = connection.new_event_queue();
    let qh = queue.handle();
    let _registry: WlRegistry = display.get_registry(&qh, ());
    let mut state = State::default();
    roundtrip(&mut queue, &mut state)?;

    let Some(screencopy) = state.screencopy.clone() else {
        return super::kwin::screen()
            .map_err(|error| format!("the compositor does not offer wlr-screencopy ({error})"));
    };
    if state.shm.is_none() {
        return Err("the compositor does not offer wl_shm".into());
    }
    if state.outputs.is_empty() {
        return Err("the compositor reported no outputs".into());
    }
    state.geometry = (0..state.outputs.len())
        .map(|_| LogicalGeometry::default())
        .collect();
    if let Some(manager) = state.xdg_output.clone() {
        for (index, output) in state.outputs.iter().enumerate() {
            manager.get_xdg_output(output, &qh, index);
        }
        roundtrip(&mut queue, &mut state)?;
    }

    for (index, output) in state.outputs.iter().enumerate() {
        screencopy.capture_output(0, output, &qh, index);
        state.frames.push(Frame::default());
    }
    while state
        .frames
        .iter()
        .any(|frame| !frame.ready && !frame.failed)
    {
        roundtrip(&mut queue, &mut state)?;
    }

    struct Piece {
        x: i32,
        y: i32,
        shot: Shot,
    }
    let mut pieces = Vec::new();
    for (index, frame) in state.frames.iter_mut().enumerate() {
        if !frame.ready {
            return Err(format!("the compositor failed to capture output {index}"));
        }
        let shot = frame_pixels(frame)?;
        if let Some(buffer) = frame.buffer.take() {
            buffer.destroy();
        }
        let geometry = &state.geometry[index];
        let (mut x, mut y) = (geometry.x, geometry.y);
        if !geometry.done || geometry.width <= 0 || geometry.height <= 0 {
            x = 0;
            y = 0;
        } else {
            x = (x as f64 * shot.width as f64 / geometry.width as f64).round() as i32;
            y = (y as f64 * shot.height as f64 / geometry.height as f64).round() as i32;
        }
        pieces.push(Piece { x, y, shot });
    }
    let min_x = pieces.iter().map(|p| p.x).min().unwrap_or(0);
    let min_y = pieces.iter().map(|p| p.y).min().unwrap_or(0);
    let max_x = pieces
        .iter()
        .map(|p| p.x + p.shot.width as i32)
        .max()
        .unwrap_or(0);
    let max_y = pieces
        .iter()
        .map(|p| p.y + p.shot.height as i32)
        .max()
        .unwrap_or(0);
    let (width, height) = ((max_x - min_x) as u32, (max_y - min_y) as u32);
    if width == 0 || height == 0 {
        return Err("captured an empty desktop".into());
    }
    let mut canvas = Shot {
        width,
        height,
        pixels: vec![0; width as usize * height as usize * 4],
    };
    for piece in pieces {
        canvas.blit(
            (piece.x - min_x, piece.y - min_y),
            (piece.shot.width, piece.shot.height),
            &piece.shot,
        );
    }
    Ok(canvas)
}
