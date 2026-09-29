//! Optional timestamp readback. Never blocks a frame waiting for the GPU.
use std::sync::{
    Arc,
    atomic::{AtomicU8, Ordering},
};
pub struct GpuTiming {
    pub queries: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    read: wgpu::Buffer,
    state: Arc<AtomicU8>,
    pending: bool,
    pub milliseconds: Option<f64>,
}
impl GpuTiming {
    pub fn new(device: &wgpu::Device) -> Option<Self> {
        if !device.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
            return None;
        }
        Some(Self {
            queries: device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("frame-timestamps"),
                ty: wgpu::QueryType::Timestamp,
                count: 2,
            }),
            resolve: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("timestamp-resolve"),
                size: 256,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }),
            read: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("timestamp-read"),
                size: 16,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            state: Arc::new(AtomicU8::new(0)),
            pending: false,
            milliseconds: None,
        })
    }
    pub fn poll(&mut self, device: &wgpu::Device, period: f32) {
        let _ = device.poll(wgpu::PollType::Poll);
        match self.state.swap(0, Ordering::Acquire) {
            1 => {
                let data = self.read.slice(..).get_mapped_range();
                let a = u64::from_le_bytes(data[0..8].try_into().unwrap());
                let b = u64::from_le_bytes(data[8..16].try_into().unwrap());
                self.milliseconds = Some(b.saturating_sub(a) as f64 * f64::from(period) / 1e6);
                drop(data);
                self.read.unmap();
                self.pending = false;
            }
            2 => {
                self.pending = false;
                self.milliseconds = None;
            }
            _ => {}
        }
    }
    pub fn ready(&self) -> bool {
        !self.pending
    }
    pub fn resolve(&self, encoder: &mut wgpu::CommandEncoder) {
        encoder.resolve_query_set(&self.queries, 0..2, &self.resolve, 0);
        encoder.copy_buffer_to_buffer(&self.resolve, 0, &self.read, 0, 16);
    }
    pub fn map(&mut self) {
        self.pending = true;
        let state = self.state.clone();
        self.read
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |r| {
                state.store(if r.is_ok() { 1 } else { 2 }, Ordering::Release)
            });
    }
}
