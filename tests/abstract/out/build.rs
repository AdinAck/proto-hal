use model::model;
use proto_hal_build::Error;

fn main() -> Result<(), Error> {
    proto_hal_build::render(&model()?);

    Ok(())
}
