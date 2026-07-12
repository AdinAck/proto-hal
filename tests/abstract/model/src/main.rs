//! Evaluate the test device's model description and report every diagnostic
//! from every phase.

use std::process::ExitCode;

use proto_hal_build::model::{Sources, evaluate_sources, report};

fn main() -> ExitCode {
    let sources = Sources::single("device.phm", abstract_model::DEVICE);

    let evaluation = evaluate_sources(&sources);

    report(&sources, &evaluation.diagnostics);

    if evaluation.failed() {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
