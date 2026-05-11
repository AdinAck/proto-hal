use std::process::ExitCode;

use abstract_model::compose;

fn main() -> ExitCode {
    phm::validate(compose())
}
