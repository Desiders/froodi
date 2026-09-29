#[rustfmt::skip]
macro_rules! all_the_tuples {
    ($name:ident) => {
        $name!([]);
        $name!([T1]);
        $name!([T1, T2]);
        $name!([T1, T2, T3]);
        $name!([T1, T2, T3, T4]);
        $name!([T1, T2, T3, T4, T5]);
        $name!([T1, T2, T3, T4, T5, T6]);
        $name!([T1, T2, T3, T4, T5, T6, T7]);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8]);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9]);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10]);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11]);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12]);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13]);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14]);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15]);
        $name!([T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16]);
    };
}

/// Like `all_the_tuples!`, but every element comes with a second identifier, for impls that
/// pair each tuple element with its own type parameter (a dependency and its index).
#[rustfmt::skip]
macro_rules! all_the_tuple_pairs {
    ($name:ident) => {
        $name!([]);
        $name!([T1 I1]);
        $name!([T1 I1, T2 I2]);
        $name!([T1 I1, T2 I2, T3 I3]);
        $name!([T1 I1, T2 I2, T3 I3, T4 I4]);
        $name!([T1 I1, T2 I2, T3 I3, T4 I4, T5 I5]);
        $name!([T1 I1, T2 I2, T3 I3, T4 I4, T5 I5, T6 I6]);
        $name!([T1 I1, T2 I2, T3 I3, T4 I4, T5 I5, T6 I6, T7 I7]);
        $name!([T1 I1, T2 I2, T3 I3, T4 I4, T5 I5, T6 I6, T7 I7, T8 I8]);
        $name!([T1 I1, T2 I2, T3 I3, T4 I4, T5 I5, T6 I6, T7 I7, T8 I8, T9 I9]);
        $name!([T1 I1, T2 I2, T3 I3, T4 I4, T5 I5, T6 I6, T7 I7, T8 I8, T9 I9, T10 I10]);
        $name!([T1 I1, T2 I2, T3 I3, T4 I4, T5 I5, T6 I6, T7 I7, T8 I8, T9 I9, T10 I10, T11 I11]);
        $name!([T1 I1, T2 I2, T3 I3, T4 I4, T5 I5, T6 I6, T7 I7, T8 I8, T9 I9, T10 I10, T11 I11, T12 I12]);
        $name!([T1 I1, T2 I2, T3 I3, T4 I4, T5 I5, T6 I6, T7 I7, T8 I8, T9 I9, T10 I10, T11 I11, T12 I12, T13 I13]);
        $name!([T1 I1, T2 I2, T3 I3, T4 I4, T5 I5, T6 I6, T7 I7, T8 I8, T9 I9, T10 I10, T11 I11, T12 I12, T13 I13, T14 I14]);
        $name!([T1 I1, T2 I2, T3 I3, T4 I4, T5 I5, T6 I6, T7 I7, T8 I8, T9 I9, T10 I10, T11 I11, T12 I12, T13 I13, T14 I14, T15 I15]);
        $name!([T1 I1, T2 I2, T3 I3, T4 I4, T5 I5, T6 I6, T7 I7, T8 I8, T9 I9, T10 I10, T11 I11, T12 I12, T13 I13, T14 I14, T15 I15, T16 I16]);
    };
}
