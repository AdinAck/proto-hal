use proto_hal_build::model::{Field, Register, model::PeripheralEntry};

pub fn dr<'cx>(crc: &mut PeripheralEntry<'cx>) {
    let mut dr = crc.add_register(Register::new("dr", 0));

    dr.add_read_write_field(Field::new("dr", 0, 32));
}
