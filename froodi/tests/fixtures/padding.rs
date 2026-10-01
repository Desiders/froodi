macro_rules! padding {
    () => { froodi::compiled_registry! { provide(App, || Ok::<_, InstantiateErrorKind>(())) } };
    (x $($rest:tt)*) => {
        froodi::compiled_registry! { extend(padding!($($rest)*), padding!($($rest)*)) }
    };
}

#[allow(unused_macros)]
macro_rules! padding_1022 {
    () => {
        froodi::compiled_registry! {
            extend(
                padding!(x x x x x x x x x), padding!(x x x x x x x x),
                padding!(x x x x x x x), padding!(x x x x x x), padding!(x x x x x),
                padding!(x x x x), padding!(x x x), padding!(x x), padding!(x),
            )
        }
    };
}

#[allow(unused_macros)]
macro_rules! padding_1021 {
    () => {
        froodi::compiled_registry! {
            extend(
                padding!(x x x x x x x x x), padding!(x x x x x x x x),
                padding!(x x x x x x x), padding!(x x x x x x), padding!(x x x x x),
                padding!(x x x x), padding!(x x x), padding!(x x), padding!(),
            )
        }
    };
}
