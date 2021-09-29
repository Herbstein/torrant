use std::convert::TryInto;

use bytes::{Buf, BytesMut};
use thiserror::Error;

pub trait ReadExactExt {
    fn read_exact<const N: usize>(&mut self) -> Option<[u8; N]>;
}

#[derive(Debug, Error)]
pub enum ReadExactError {
    #[error("not enough bytes in buffer, expected {0}")]
    NotEnoughRemaining(usize),
}

impl ReadExactExt for BytesMut {
    fn read_exact<const N: usize>(&mut self) -> Option<[u8; N]> {
        if self.remaining() < N {
            return None;
        }

        Some(self.get(0..N).unwrap().try_into().unwrap())
    }
}
