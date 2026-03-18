use abstract_model::model;
use proto_hal_model::error::Error;

fn main() -> Result<(), Error> {
    proto_hal_model::validate(&model()?);

    Ok(())
}
