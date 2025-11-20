use proto_hal_build::model::structures::{
    field::Field, model::PeripheralEntry, register::Register,
};

pub fn dr<'cx>(crc: &mut PeripheralEntry<'cx>) {
    let mut dr = crc.add_register(Register::new("dr", 0));

    dr.add_read_write_field(Field::new("dr", 0, 32));
}
