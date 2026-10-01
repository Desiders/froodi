#[macro_export]
macro_rules! compiled_registry {
    ($($tokens:tt)*) => {
        $crate::macros_utils::compiled::registry!($crate::macros_utils::compiled; $($tokens)*)
    };
}

#[cfg(feature = "async")]
#[macro_export]
macro_rules! compiled_async_registry {
    ($($tokens:tt)*) => {
        $crate::macros_utils::compiled::async_registry!($crate::macros_utils::compiled; $($tokens)*)
    };
}
