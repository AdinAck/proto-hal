pub mod enr;

use proto_hal_build::model::structures::{
    entitlement::Entitlement, model::Model, peripheral::Peripheral,
};

use crate::rcc::enr::enr;

// TODO: improve this
pub struct Output {
    cordicen: Entitlement,
    crcen: Entitlement,
}

pub fn rcc(model: &mut Model) -> Output {
    let mut rcc = model.add_peripheral(Peripheral::new("rcc", 0x4002_1000));

    enr(&mut rcc, enr::Instance::AHB1);
    enr(&mut rcc, enr::Instance::AHB2);
}
