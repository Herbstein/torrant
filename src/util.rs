use std::{
    convert::TryInto,
    io::{Cursor, Read},
};

use bytes::{Buf, BytesMut};
use thiserror::Error;

pub trait ReadExactExt {
    fn read_exact_arr<const N: usize>(&mut self) -> Option<[u8; N]>;
}

#[derive(Debug, Error)]
pub enum ReadExactError {
    #[error("not enough bytes in buffer, expected {0}")]
    NotEnoughRemaining(usize),
}

impl ReadExactExt for BytesMut {
    fn read_exact_arr<const N: usize>(&mut self) -> Option<[u8; N]> {
        if self.remaining() < N {
            return None;
        }

        Some(self.get(0..N).unwrap().try_into().unwrap())
    }
}

impl<T> ReadExactExt for Cursor<T>
where
    T: AsRef<[u8]>,
{
    fn read_exact_arr<const N: usize>(&mut self) -> Option<[u8; N]> {
        let mut arr = [0; N];
        self.read_exact(&mut arr).ok().map(|_| arr)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use crate::util::ReadExactExt;

    #[test]
    fn test_read_exact() {
        let vec = vec![0u8; 50];
        let mut cursor = Cursor::new(vec);

        let read_arr = cursor.read_exact_arr::<5>();
        assert!(matches!(read_arr, Some(_)));

        let read_arr = read_arr.unwrap();
        assert_eq!(read_arr.len(), 5);
    }
}
