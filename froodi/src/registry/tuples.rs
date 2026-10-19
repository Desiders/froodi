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
