use proto_hal_build::model::structures::{
    field::Field, model::PeripheralEntry, register::Register,
};

pub fn idr<'cx>(crc: &mut PeripheralEntry<'cx>) {
    let mut idr = crc.add_register(Register::new("idr", 4).reset(0));

    idr.add_store_field(Field::new("idr", 0, 32));
}
