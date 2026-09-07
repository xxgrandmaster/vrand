
use sha2::{Digest, Sha256};
use std::collections::VecDeque;

pub const BUFFER_SIZE: usize = 262_144; // 256 KiB

pub struct RollingBuffer {
    data: VecDeque<u8>,
    capacity: usize,
}

impl RollingBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            data: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub fn push(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.data.push_back(byte);

            if self.data.len() > self.capacity {
                self.data.pop_front();
            }
        }
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn sha256(&mut self) -> [u8; 32] {
        let data = self.data.make_contiguous();

        let mut hasher = Sha256::new();
        hasher.update(&*data);

        hasher.finalize().into()
    }
}

#[cfg(test)]
mod tests {
    use super::RollingBuffer;

    #[test]
    fn keeps_only_latest_bytes() {
        let mut buffer = RollingBuffer::new(4);

        buffer.push(b"abcd");
        buffer.push(b"ef");

        assert_eq!(buffer.len(), 4);
        assert_eq!(buffer.data.iter().copied().collect::<Vec<_>>(), b"cdef");
    }

    #[test]
    fn computes_sha256_for_current_contents() {
        let mut buffer = RollingBuffer::new(16);
        buffer.push(b"hello");

        let digest = buffer.sha256();
        assert_eq!(digest.len(), 32);
        assert_ne!(digest, [0u8; 32]);
    }
}
