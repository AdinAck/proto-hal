pub mod cr;
pub mod dr;
pub mod idr;

use proto_hal_model::{Entitlement, Model, Peripheral, error::Error};

use cr::cr;
use dr::dr;
use idr::idr;

pub fn crc(model: &mut Model, crcen: Entitlement) -> Result<(), Error> {
    let mut crc = model.add_peripheral(Peripheral::new("crc", 0x4002_3000));
    crc.ontological_entitlements([[crcen]])?;

    dr(&mut crc);
    idr(&mut crc);
    cr(&mut crc);

    Ok(())
}
