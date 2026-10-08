#[macro_export]
macro_rules! registry {
    ($($tokens:tt)*) => {
        $crate::macros_utils::typed::registry!($crate::macros_utils::typed; $($tokens)*)
    };
}
