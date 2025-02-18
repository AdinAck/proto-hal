macro_rules! success {
    ($($arg:tt)*) => {
        format!(
            "{}: {}",
            ::colored::Colorize::bold(::colored::Colorize::green("success")),
            format!($($arg)*),
        )
    };
}

pub(crate) use success;
