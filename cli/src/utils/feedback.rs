macro_rules! error {
    ($($arg:tt)*) => {
        format!(
            "{}: {}",
            ::colored::Colorize::bold(::colored::Colorize::red("error")),
            format!($($arg)*),
        )
    };
}

macro_rules! warning {
    ($($arg:tt)*) => {
        format!(
            "{}: {}",
            ::colored::Colorize::bold(::colored::Colorize::yellow("warning")),
            format!($($arg)*),
        )
    };
}

macro_rules! success {
    ($($arg:tt)*) => {
        format!(
            "{}: {}",
            ::colored::Colorize::bold(::colored::Colorize::green("success")),
            format!($($arg)*),
        )
    };
}

pub(crate) use error;
pub(crate) use success;
pub(crate) use warning;
