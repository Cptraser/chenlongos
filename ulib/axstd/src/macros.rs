//! Standard library macros

/// Prints to the standard output.
///
/// Equivalent to the [`println!`] macro except that a newline is not printed at
/// the end of the message.
///
/// [`println!`]: crate::println
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        $crate::io::__print_impl(format_args!($($arg)*));
    }
}

/// Prints to the standard output, with a newline.
#[macro_export]
macro_rules! println {
    () => { $crate::print!("\n") };
    ($($arg:tt)*) => {
        $crate::io::__print_impl(format_args!("{}\n", format_args!($($arg)*)));
    }
}

#[macro_export]
macro_rules! pinfo {
    ($($arg:tt)*) => {
        $crate::io::__print_impl_debug(1, format_args!("\u{1B}[{}m[INFO]\u{1B}[m  {}\n", 92 as u8, format_args!($($arg)*)));
    }
}

#[macro_export]
macro_rules! pdev {
    ($($arg:tt)*) => {
        $crate::io::__print_impl_debug(2, format_args!("\u{1B}[{}m[DEV]\u{1B}[m   {}\n", 94 as u8, format_args!($($arg)*)));
    }
}

#[macro_export]
macro_rules! pdebug {
    ($($arg:tt)*) => {
        $crate::io::__print_impl_debug(3, format_args!("\u{1B}[{}m[DEBUG]\u{1B}[m {}\n", 93 as u8, format_args!($($arg)*)));
    }
}
