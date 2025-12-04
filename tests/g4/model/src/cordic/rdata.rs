use proto_hal_model::{Entitlement, Register, model::PeripheralEntry};

pub mod res;
pub mod res0;
pub mod res1;

use res::res;
use res0::res0;
use res1::res1;

pub struct Entitlements {
    pub ressize_q15: Entitlement,
    pub ressize_q31: Entitlement,
    pub nres_one: Entitlement,
}

pub fn rdata<'cx>(cordic: &mut PeripheralEntry<'cx>, entitlements: Entitlements) {
    let mut rdata = cordic.add_register(Register::new("rdata", 8));

    res(&mut rdata, entitlements.ressize_q31);
    res0(&mut rdata, entitlements.ressize_q15, entitlements.nres_one);
    res1(&mut rdata, entitlements.ressize_q15, entitlements.nres_one);
}
