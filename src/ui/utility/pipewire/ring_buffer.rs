use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct RingBuffer {
    data: Box<[f32]>,
    head: Arc<AtomicUsize>,
    len: Arc<AtomicUsize>,
}

impl RingBuffer {
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            data: vec![0.0; capacity].into_boxed_slice(),
            head: Arc::new(AtomicUsize::new(0)),
            len: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Clone of the internal length counter
    pub(crate) fn len_handle(&self) -> Arc<AtomicUsize> {
        self.len.clone()
    }

    pub(crate) fn head_handle(&self) -> Arc<AtomicUsize> {
        self.head.clone()
    }

    pub(crate) fn clear(&mut self) {
        self.head.store(0, Ordering::Release);
        self.len.store(0, Ordering::Release);
    }

    fn capacity(&self) -> usize {
        self.data.len()
    }

    /// Write as many samples from `src` as fit, batched via copy_from_slice.
    pub(crate) fn write(&mut self, src: &[f32]) -> usize {
        let current_len = self.len.load(Ordering::Relaxed);
        let current_head = self.head.load(Ordering::Relaxed);
        let available = self.capacity() - current_len;
        let count = src.len().min(available);
        let capacity = self.capacity();
        let tail = (current_head + current_len) % capacity;
        let first = count.min(capacity - tail);

        self.data[tail..tail + first].copy_from_slice(&src[..first]);
        if first < count {
            self.data[..count - first].copy_from_slice(&src[first..count]);
        }
        self.len.store(current_len + count, Ordering::Release);
        count
    }

    /// Read+rotate `count` samples into `dst`, looping if fewer are buffered.
    pub(crate) fn read_looped(&mut self, dest: &mut [f32]) {
        let count = dest.len();
        let loop_len = self.len.load(Ordering::Relaxed);

        if loop_len == 0 {
            dest.fill(0.0);
            return;
        }

        let mut written = 0;
        let mut head = self.head.load(Ordering::Relaxed);
        while written < count {
            let read_pos = head % loop_len;
            let chunk = (loop_len - read_pos).min(count - written);
            dest[written..written + chunk].copy_from_slice(&self.data[read_pos..read_pos + chunk]);
            written += chunk;
            head = (read_pos + chunk) % loop_len;
        }
        self.head.store(head, Ordering::Release);
    }
}
