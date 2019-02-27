//! Stream module

use super::AbiType;
use crate::errors::AbiError;

/// Stream interpretation of incoming payload
pub struct Stream<'a> {
    payload: &'a [u8],
    position: usize,
}

impl<'a> Stream<'a> {
    /// New stream for known payload
    pub fn new(raw: &'a [u8]) -> Self {
        Stream {
            payload: raw,
            position: 0,
        }
    }

    /// Pop next argument of known type
    pub fn pop<T: AbiType>(&mut self) -> Result<T, AbiError> {
        if T::IS_FIXED {
            T::decode(self)
        } else {
            let offset = u32::decode(self)?;
            let mut nested_stream = Stream::new(&self.payload[offset as usize..]);
            T::decode(&mut nested_stream)
        }
    }

    /// Current position for the stream
    pub fn position(&self) -> usize {
        self.position
    }

    /// Advance stream position for `amount` bytes
    pub fn advance(&mut self, amount: usize) -> Result<usize, AbiError> {
        if self.position + amount > self.payload.len() {
            return Err(AbiError::UnexpectedEof);
        }

        let old_position = self.position;
        self.position += amount;
        Ok(old_position)
    }

    /// Finish current advance, advancing stream to the next 32 byte step
    pub fn finish_advance(&mut self) {
        if self.position % 32 > 0 {
            self.position += 32 - (self.position % 32);
        }
    }

    /// Stream payload
    pub fn payload(&self) -> &[u8] {
        self.payload
    }

    /// Peek next byte in stream
    pub fn peek(&self) -> u8 {
        self.payload[self.position]
    }
}
