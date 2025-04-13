#![no_std]

include!(concat!(env!("OUT_DIR"), "/hal.rs"));

#[cfg(test)]
mod tests {

    mod peripherals {
        use crate::{bar, foo};

        #[test]
        fn base_addr() {
            assert_eq!(foo::BASE_ADDR, 0);
            assert_eq!(bar::BASE_ADDR, 0x100);
        }
    }

    mod registers {
        use crate::{bar::bar0, foo::foo0};

        #[test]
        fn offset() {
            assert_eq!(foo0::OFFSET, 0);
            assert_eq!(bar0::OFFSET, 0);
        }
    }
}
