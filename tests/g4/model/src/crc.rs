pub mod cr;
pub mod dr;
pub mod idr;

use proto_hal_build::model::structures::{
    entitlement::Entitlement, model::Model, peripheral::Peripheral,
};

use cr::cr;
use dr::dr;
use idr::idr;

pub fn crc(model: &mut Model, crcen: Entitlement) {
    let mut crc = model.add_peripheral(Peripheral::new("crc", 0x4002_3000));
    crc.ontological_entitlements([crcen]);

    dr(&mut crc);
    idr(&mut crc);
    cr(&mut crc);
}
