//! The [word size](crate#word-size) of a device.

use std::ops::RangeInclusive;

use derive_more::Display;

use crate::{Word, field::Domain};

/// A permitted [word size](crate#word-size). (*See [`WordSize::SIZES`]*)
#[derive(Debug, Clone, Copy, Display)]
pub struct WordSize(u8);

impl WordSize {
    /// The permitted word sizes, in bytes, as an address is at most one [`Word`].
    pub const SIZES: RangeInclusive<Word> = 1..=(Word::BITS / 8) as Word;

    /// Create a word size of the provided number of bytes, if the number is [permitted](Self::SIZES).
    pub fn new(size: Word) -> Option<Self> {
        Self::SIZES
            .contains(&size)
            .then(|| Self(u8::try_from(size).expect("valid word sizes fit in a byte")))
    }

    /// The word size, in bytes.
    pub fn bytes(self) -> u8 {
        self.0
    }

    /// The field [domain](crate#domain) spanning an entire register of this word size.
    pub(crate) fn domain(self) -> Domain {
        Domain::new(self.bytes() * 8 - 1, 0).expect("a word has bits")
    }

    /// The last address of the [address space](crate#address-space).
    pub(crate) fn last_address(self) -> Word {
        // invariant: a valid word size is at most `Word::BITS / 8` bytes, so the shift is less than `Word::BITS`
        Word::MAX >> (Word::BITS - u32::from(self.bytes()) * 8)
    }
}
