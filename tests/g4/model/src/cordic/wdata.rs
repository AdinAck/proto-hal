pub mod arg;
pub mod arg0;
pub mod arg1;

use proto_hal_model::{Entitlement, Register, model::PeripheralEntry};

use arg::arg;
use arg0::arg0;
use arg1::arg1;

pub struct Entitlements {
    pub argsize_q15: Entitlement,
    pub argsize_q31: Entitlement,
    pub nargs_one: Entitlement,
}

pub fn wdata<'cx>(cordic: &mut PeripheralEntry<'cx>, entitlements: Entitlements) {
    let mut wdata = cordic.add_register(Register::new("wdata", 4));

    arg(&mut wdata, entitlements.argsize_q31);
    arg0(&mut wdata, entitlements.argsize_q15, entitlements.nargs_one);
    arg1(&mut wdata, entitlements.argsize_q15, entitlements.nargs_one);
}
