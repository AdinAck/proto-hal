pub mod ahb1enr;
pub mod ahb2enr;

use proto_hal_build::model::structures::{
    entitlement::Entitlement, model::Model, peripheral::Peripheral,
};

use crate::rcc::{ahb1enr::ahb1enr, ahb2enr::ahb2enr};

// TODO: improve this
pub struct Output {
    pub cordicen: Entitlement,
    pub crcen: Entitlement,
}

pub fn rcc(model: &mut Model) -> Output {
    let mut rcc = model.add_peripheral(Peripheral::new("rcc", 0x4002_1000));

    let ahb1enr::Output { cordicen, crcen } = ahb1enr(&mut rcc);
    ahb2enr(&mut rcc);

    Output { cordicen, crcen }
}
