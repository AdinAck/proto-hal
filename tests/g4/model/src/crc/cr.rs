pub mod polysize;
pub mod rev_in;
pub mod rev_out;
pub mod rst;

use proto_hal_model::{Register, model::PeripheralEntry};

use polysize::polysize;
use rev_in::rev_in;
use rev_out::rev_out;
use rst::rst;

pub fn cr<'cx>(crc: &mut PeripheralEntry<'cx>) {
    let mut cr = crc.add_register(Register::new("cr", 8).reset(0));

    rst(&mut cr);
    polysize(&mut cr);
    rev_in(&mut cr);
    rev_out(&mut cr);
}
