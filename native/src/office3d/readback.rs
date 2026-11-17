//! Frames come back without waiting on the GPU: two readback buffers in turn.
//! A frame is drawn into a free one and asked to map; on a later paint, once
//! the GPU has finished, its bytes are taken. The UI thread never blocks on
//! the card, and the picture runs a frame behind, as nobody can see.

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

const FREE: u8 = 0;
const PENDING: u8 = 1;
const READY: u8 = 2;

struct Slot {
    buffer: wgpu::Buffer,
    state: Arc<AtomicU8>,
    /// Which frame it holds: the newest ready one wins.
    seq: u64,
}

pub(super) struct Readback {
    slots: Vec<Slot>,
    /// Bytes per row in the buffers: a multiple of 256, as wgpu wants.
    pub row: u32,
    pub width: u32,
    pub height: u32,
    next: u64,
}

impl Readback {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let row = (width * 4).div_ceil(256) * 256;
        let slot = || Slot { buffer: device.create_buffer(&wgpu::BufferDescriptor { label: Some("readback"), size: (row * height) as u64, usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ, mapped_at_creation: false }), state: Arc::new(AtomicU8::new(FREE)), seq: 0 };
        Self { slots: vec![slot(), slot()], row, width, height, next: 1 }
    }

    /// A buffer the next frame can be copied into, if one is free.
    pub fn free(&self) -> Option<usize> {
        self.slots.iter().position(|s| s.state.load(Ordering::Acquire) == FREE)
    }

    /// A frame is on its way back from the card.
    pub fn busy(&self) -> bool {
        self.slots.iter().any(|s| s.state.load(Ordering::Acquire) == PENDING)
    }

    pub fn buffer(&self, slot: usize) -> &wgpu::Buffer {
        &self.slots[slot].buffer
    }

    /// The frame in `slot` was submitted: ask for it to map.
    pub fn submitted(&mut self, slot: usize) {
        let s = &mut self.slots[slot];
        s.seq = self.next;
        self.next += 1;
        s.state.store(PENDING, Ordering::Release);
        let state = s.state.clone();
        s.buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| state.store(if r.is_ok() { READY } else { FREE }, Ordering::Release));
    }

    /// The newest finished frame's BGRA bytes, rows packed tight; older
    /// finished ones are let go unread.
    pub fn take(&mut self) -> Option<Vec<u8>> {
        let ready: Vec<usize> = (0..self.slots.len()).filter(|&i| self.slots[i].state.load(Ordering::Acquire) == READY).collect();
        let newest = *ready.iter().max_by_key(|&&i| self.slots[i].seq)?;
        let mut out = None;
        for i in ready {
            let s = &self.slots[i];
            if i == newest {
                if let Ok(data) = s.buffer.slice(..).get_mapped_range() {
                    let mut bytes = Vec::with_capacity((self.width * self.height * 4) as usize);
                    for y in 0..self.height as usize {
                        let start = y * self.row as usize;
                        bytes.extend_from_slice(&data[start..start + self.width as usize * 4]);
                    }
                    out = Some(bytes);
                }
            }
            s.buffer.unmap();
            s.state.store(FREE, Ordering::Release);
        }
        out
    }
}
