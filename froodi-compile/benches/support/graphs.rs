//! Checked-in fixtures keep the same graph shapes across engines and scale tests.
//!
//! Current Froodi against the compile-time engine's static and runtime registries (issues #59, #64):
//!
//! - `froodi`: the current runtime engine;
//! - `static`: static edges use rustc-resolved IDs in the executor table;
//! - `indexed`: the same registry erased with `into_runtime()`, edges go through the
//!   construction table by registration id with a checked downcast.
#![allow(clippy::all, clippy::pedantic, dead_code)]

use std::sync::Arc;

#[derive(Clone)]
pub struct S0;
pub struct S1(pub Arc<dyn Send + Sync>);
pub struct S2(pub Arc<dyn Send + Sync>);
pub struct S3(pub Arc<dyn Send + Sync>);
pub struct S4(pub Arc<dyn Send + Sync>);
pub struct S5(pub Arc<dyn Send + Sync>);
pub struct S6(pub Arc<dyn Send + Sync>);
pub struct S7(pub Arc<dyn Send + Sync>);
pub struct S8(pub Arc<dyn Send + Sync>);
pub struct S9(pub Arc<dyn Send + Sync>);
pub struct S10(pub Arc<dyn Send + Sync>);
pub struct S11(pub Arc<dyn Send + Sync>);
pub struct S12(pub Arc<dyn Send + Sync>);
pub struct S13(pub Arc<dyn Send + Sync>);
pub struct S14(pub Arc<dyn Send + Sync>);
pub struct S15(pub Arc<dyn Send + Sync>);
pub struct S16(pub Arc<dyn Send + Sync>);
pub struct S17(pub Arc<dyn Send + Sync>);
pub struct S18(pub Arc<dyn Send + Sync>);
pub struct S19(pub Arc<dyn Send + Sync>);
pub struct S20(pub Arc<dyn Send + Sync>);
pub struct S21(pub Arc<dyn Send + Sync>);
pub struct S22(pub Arc<dyn Send + Sync>);
pub struct S23(pub Arc<dyn Send + Sync>);
pub struct S24(pub Arc<dyn Send + Sync>);
pub struct S25(pub Arc<dyn Send + Sync>);
pub struct S26(pub Arc<dyn Send + Sync>);
pub struct S27(pub Arc<dyn Send + Sync>);
pub struct S28(pub Arc<dyn Send + Sync>);
pub struct S29(pub Arc<dyn Send + Sync>);
pub struct S30(pub Arc<dyn Send + Sync>);
pub struct S31(pub Arc<dyn Send + Sync>);
pub struct S32(pub Arc<dyn Send + Sync>);
pub struct S33(pub Arc<dyn Send + Sync>);
pub struct S34(pub Arc<dyn Send + Sync>);
pub struct S35(pub Arc<dyn Send + Sync>);
pub struct S36(pub Arc<dyn Send + Sync>);
pub struct S37(pub Arc<dyn Send + Sync>);
pub struct S38(pub Arc<dyn Send + Sync>);
pub struct S39(pub Arc<dyn Send + Sync>);
pub struct S40(pub Arc<dyn Send + Sync>);
pub struct S41(pub Arc<dyn Send + Sync>);
pub struct S42(pub Arc<dyn Send + Sync>);
pub struct S43(pub Arc<dyn Send + Sync>);
pub struct S44(pub Arc<dyn Send + Sync>);
pub struct S45(pub Arc<dyn Send + Sync>);
pub struct S46(pub Arc<dyn Send + Sync>);
pub struct S47(pub Arc<dyn Send + Sync>);
pub struct S48(pub Arc<dyn Send + Sync>);
pub struct S49(pub Arc<dyn Send + Sync>);
pub struct S50(pub Arc<dyn Send + Sync>);
pub struct S51(pub Arc<dyn Send + Sync>);
pub struct S52(pub Arc<dyn Send + Sync>);
pub struct S53(pub Arc<dyn Send + Sync>);
pub struct S54(pub Arc<dyn Send + Sync>);
pub struct S55(pub Arc<dyn Send + Sync>);
pub struct S56(pub Arc<dyn Send + Sync>);
pub struct S57(pub Arc<dyn Send + Sync>);
pub struct S58(pub Arc<dyn Send + Sync>);
pub struct S59(pub Arc<dyn Send + Sync>);
pub struct S60(pub Arc<dyn Send + Sync>);
pub struct S61(pub Arc<dyn Send + Sync>);
pub struct S62(pub Arc<dyn Send + Sync>);
pub struct S63(pub Arc<dyn Send + Sync>);
pub struct S64(pub Arc<dyn Send + Sync>);
pub struct S65(pub Arc<dyn Send + Sync>);
pub struct S66(pub Arc<dyn Send + Sync>);
pub struct S67(pub Arc<dyn Send + Sync>);
pub struct S68(pub Arc<dyn Send + Sync>);
pub struct S69(pub Arc<dyn Send + Sync>);
pub struct S70(pub Arc<dyn Send + Sync>);
pub struct S71(pub Arc<dyn Send + Sync>);
pub struct S72(pub Arc<dyn Send + Sync>);
pub struct S73(pub Arc<dyn Send + Sync>);
pub struct S74(pub Arc<dyn Send + Sync>);
pub struct S75(pub Arc<dyn Send + Sync>);
pub struct S76(pub Arc<dyn Send + Sync>);
pub struct S77(pub Arc<dyn Send + Sync>);
pub struct S78(pub Arc<dyn Send + Sync>);
pub struct S79(pub Arc<dyn Send + Sync>);
pub struct S80(pub Arc<dyn Send + Sync>);
pub struct S81(pub Arc<dyn Send + Sync>);
pub struct S82(pub Arc<dyn Send + Sync>);
pub struct S83(pub Arc<dyn Send + Sync>);
pub struct S84(pub Arc<dyn Send + Sync>);
pub struct S85(pub Arc<dyn Send + Sync>);
pub struct S86(pub Arc<dyn Send + Sync>);
pub struct S87(pub Arc<dyn Send + Sync>);
pub struct S88(pub Arc<dyn Send + Sync>);
pub struct S89(pub Arc<dyn Send + Sync>);
pub struct S90(pub Arc<dyn Send + Sync>);
pub struct S91(pub Arc<dyn Send + Sync>);
pub struct S92(pub Arc<dyn Send + Sync>);
pub struct S93(pub Arc<dyn Send + Sync>);
pub struct S94(pub Arc<dyn Send + Sync>);
pub struct S95(pub Arc<dyn Send + Sync>);
pub struct S96(pub Arc<dyn Send + Sync>);
pub struct S97(pub Arc<dyn Send + Sync>);
pub struct S98(pub Arc<dyn Send + Sync>);
pub struct S99(pub Arc<dyn Send + Sync>);
#[derive(Clone)]
pub struct T0;
pub struct T1(pub Box<dyn Send + Sync>);
pub struct T2(pub Box<dyn Send + Sync>);
pub struct T3(pub Box<dyn Send + Sync>);
pub struct T4(pub Box<dyn Send + Sync>);
pub struct T5(pub Box<dyn Send + Sync>);
pub struct T6(pub Box<dyn Send + Sync>);
pub struct T7(pub Box<dyn Send + Sync>);
pub struct T8(pub Box<dyn Send + Sync>);
pub struct T9(pub Box<dyn Send + Sync>);
pub struct T10(pub Box<dyn Send + Sync>);
pub struct T11(pub Box<dyn Send + Sync>);
pub struct T12(pub Box<dyn Send + Sync>);
pub struct T13(pub Box<dyn Send + Sync>);
pub struct T14(pub Box<dyn Send + Sync>);
pub struct T15(pub Box<dyn Send + Sync>);
pub struct T16(pub Box<dyn Send + Sync>);
pub struct T17(pub Box<dyn Send + Sync>);
pub struct T18(pub Box<dyn Send + Sync>);
pub struct T19(pub Box<dyn Send + Sync>);
pub struct T20(pub Box<dyn Send + Sync>);
pub struct T21(pub Box<dyn Send + Sync>);
pub struct T22(pub Box<dyn Send + Sync>);
pub struct T23(pub Box<dyn Send + Sync>);
pub struct T24(pub Box<dyn Send + Sync>);
pub struct T25(pub Box<dyn Send + Sync>);
pub struct T26(pub Box<dyn Send + Sync>);
pub struct T27(pub Box<dyn Send + Sync>);
pub struct T28(pub Box<dyn Send + Sync>);
pub struct T29(pub Box<dyn Send + Sync>);
pub struct T30(pub Box<dyn Send + Sync>);
pub struct T31(pub Box<dyn Send + Sync>);
pub struct T32(pub Box<dyn Send + Sync>);
pub struct T33(pub Box<dyn Send + Sync>);
pub struct T34(pub Box<dyn Send + Sync>);
pub struct T35(pub Box<dyn Send + Sync>);
pub struct T36(pub Box<dyn Send + Sync>);
pub struct T37(pub Box<dyn Send + Sync>);
pub struct T38(pub Box<dyn Send + Sync>);
pub struct T39(pub Box<dyn Send + Sync>);
pub struct T40(pub Box<dyn Send + Sync>);
pub struct T41(pub Box<dyn Send + Sync>);
pub struct T42(pub Box<dyn Send + Sync>);
pub struct T43(pub Box<dyn Send + Sync>);
pub struct T44(pub Box<dyn Send + Sync>);
pub struct T45(pub Box<dyn Send + Sync>);
pub struct T46(pub Box<dyn Send + Sync>);
pub struct T47(pub Box<dyn Send + Sync>);
pub struct T48(pub Box<dyn Send + Sync>);
pub struct T49(pub Box<dyn Send + Sync>);
pub struct T50(pub Box<dyn Send + Sync>);
pub struct T51(pub Box<dyn Send + Sync>);
pub struct T52(pub Box<dyn Send + Sync>);
pub struct T53(pub Box<dyn Send + Sync>);
pub struct T54(pub Box<dyn Send + Sync>);
pub struct T55(pub Box<dyn Send + Sync>);
pub struct T56(pub Box<dyn Send + Sync>);
pub struct T57(pub Box<dyn Send + Sync>);
pub struct T58(pub Box<dyn Send + Sync>);
pub struct T59(pub Box<dyn Send + Sync>);
pub struct T60(pub Box<dyn Send + Sync>);
pub struct T61(pub Box<dyn Send + Sync>);
pub struct T62(pub Box<dyn Send + Sync>);
pub struct T63(pub Box<dyn Send + Sync>);
pub struct T64(pub Box<dyn Send + Sync>);
pub struct T65(pub Box<dyn Send + Sync>);
pub struct T66(pub Box<dyn Send + Sync>);
pub struct T67(pub Box<dyn Send + Sync>);
pub struct T68(pub Box<dyn Send + Sync>);
pub struct T69(pub Box<dyn Send + Sync>);
pub struct T70(pub Box<dyn Send + Sync>);
pub struct T71(pub Box<dyn Send + Sync>);
pub struct T72(pub Box<dyn Send + Sync>);
pub struct T73(pub Box<dyn Send + Sync>);
pub struct T74(pub Box<dyn Send + Sync>);
pub struct T75(pub Box<dyn Send + Sync>);
pub struct T76(pub Box<dyn Send + Sync>);
pub struct T77(pub Box<dyn Send + Sync>);
pub struct T78(pub Box<dyn Send + Sync>);
pub struct T79(pub Box<dyn Send + Sync>);
pub struct T80(pub Box<dyn Send + Sync>);
pub struct T81(pub Box<dyn Send + Sync>);
pub struct T82(pub Box<dyn Send + Sync>);
pub struct T83(pub Box<dyn Send + Sync>);
pub struct T84(pub Box<dyn Send + Sync>);
pub struct T85(pub Box<dyn Send + Sync>);
pub struct T86(pub Box<dyn Send + Sync>);
pub struct T87(pub Box<dyn Send + Sync>);
pub struct T88(pub Box<dyn Send + Sync>);
pub struct T89(pub Box<dyn Send + Sync>);
pub struct T90(pub Box<dyn Send + Sync>);
pub struct T91(pub Box<dyn Send + Sync>);
pub struct T92(pub Box<dyn Send + Sync>);
pub struct T93(pub Box<dyn Send + Sync>);
pub struct T94(pub Box<dyn Send + Sync>);
pub struct T95(pub Box<dyn Send + Sync>);
pub struct T96(pub Box<dyn Send + Sync>);
pub struct T97(pub Box<dyn Send + Sync>);
pub struct T98(pub Box<dyn Send + Sync>);
pub struct T99(pub Box<dyn Send + Sync>);
pub struct Wide(pub [u8; 16]);
pub struct Plugin(pub u8);
pub struct Host(pub Arc<Plugin>);

pub mod froodi_engine {
    use super::*;
    use froodi::{registry, Container, DefaultScope::*, Inject, InjectTransient, InstantiateErrorKind};

    mod f {
        use super::*;
        pub fn s0() -> Result<S0, InstantiateErrorKind> {
            Ok(S0)
        }
        pub fn t0() -> Result<T0, InstantiateErrorKind> {
            Ok(T0)
        }
        pub fn s1(Inject(p): Inject<S0>) -> Result<S1, InstantiateErrorKind> {
            Ok(S1(p))
        }
        pub fn t1(InjectTransient(p): InjectTransient<T0>) -> Result<T1, InstantiateErrorKind> {
            Ok(T1(Box::new(p)))
        }
        pub fn s2(Inject(p): Inject<S1>) -> Result<S2, InstantiateErrorKind> {
            Ok(S2(p))
        }
        pub fn t2(InjectTransient(p): InjectTransient<T1>) -> Result<T2, InstantiateErrorKind> {
            Ok(T2(Box::new(p)))
        }
        pub fn s3(Inject(p): Inject<S2>) -> Result<S3, InstantiateErrorKind> {
            Ok(S3(p))
        }
        pub fn t3(InjectTransient(p): InjectTransient<T2>) -> Result<T3, InstantiateErrorKind> {
            Ok(T3(Box::new(p)))
        }
        pub fn s4(Inject(p): Inject<S3>) -> Result<S4, InstantiateErrorKind> {
            Ok(S4(p))
        }
        pub fn t4(InjectTransient(p): InjectTransient<T3>) -> Result<T4, InstantiateErrorKind> {
            Ok(T4(Box::new(p)))
        }
        pub fn s5(Inject(p): Inject<S4>) -> Result<S5, InstantiateErrorKind> {
            Ok(S5(p))
        }
        pub fn t5(InjectTransient(p): InjectTransient<T4>) -> Result<T5, InstantiateErrorKind> {
            Ok(T5(Box::new(p)))
        }
        pub fn s6(Inject(p): Inject<S5>) -> Result<S6, InstantiateErrorKind> {
            Ok(S6(p))
        }
        pub fn t6(InjectTransient(p): InjectTransient<T5>) -> Result<T6, InstantiateErrorKind> {
            Ok(T6(Box::new(p)))
        }
        pub fn s7(Inject(p): Inject<S6>) -> Result<S7, InstantiateErrorKind> {
            Ok(S7(p))
        }
        pub fn t7(InjectTransient(p): InjectTransient<T6>) -> Result<T7, InstantiateErrorKind> {
            Ok(T7(Box::new(p)))
        }
        pub fn s8(Inject(p): Inject<S7>) -> Result<S8, InstantiateErrorKind> {
            Ok(S8(p))
        }
        pub fn t8(InjectTransient(p): InjectTransient<T7>) -> Result<T8, InstantiateErrorKind> {
            Ok(T8(Box::new(p)))
        }
        pub fn s9(Inject(p): Inject<S8>) -> Result<S9, InstantiateErrorKind> {
            Ok(S9(p))
        }
        pub fn t9(InjectTransient(p): InjectTransient<T8>) -> Result<T9, InstantiateErrorKind> {
            Ok(T9(Box::new(p)))
        }
        pub fn s10(Inject(p): Inject<S9>) -> Result<S10, InstantiateErrorKind> {
            Ok(S10(p))
        }
        pub fn t10(InjectTransient(p): InjectTransient<T9>) -> Result<T10, InstantiateErrorKind> {
            Ok(T10(Box::new(p)))
        }
        pub fn s11(Inject(p): Inject<S10>) -> Result<S11, InstantiateErrorKind> {
            Ok(S11(p))
        }
        pub fn t11(InjectTransient(p): InjectTransient<T10>) -> Result<T11, InstantiateErrorKind> {
            Ok(T11(Box::new(p)))
        }
        pub fn s12(Inject(p): Inject<S11>) -> Result<S12, InstantiateErrorKind> {
            Ok(S12(p))
        }
        pub fn t12(InjectTransient(p): InjectTransient<T11>) -> Result<T12, InstantiateErrorKind> {
            Ok(T12(Box::new(p)))
        }
        pub fn s13(Inject(p): Inject<S12>) -> Result<S13, InstantiateErrorKind> {
            Ok(S13(p))
        }
        pub fn t13(InjectTransient(p): InjectTransient<T12>) -> Result<T13, InstantiateErrorKind> {
            Ok(T13(Box::new(p)))
        }
        pub fn s14(Inject(p): Inject<S13>) -> Result<S14, InstantiateErrorKind> {
            Ok(S14(p))
        }
        pub fn t14(InjectTransient(p): InjectTransient<T13>) -> Result<T14, InstantiateErrorKind> {
            Ok(T14(Box::new(p)))
        }
        pub fn s15(Inject(p): Inject<S14>) -> Result<S15, InstantiateErrorKind> {
            Ok(S15(p))
        }
        pub fn t15(InjectTransient(p): InjectTransient<T14>) -> Result<T15, InstantiateErrorKind> {
            Ok(T15(Box::new(p)))
        }
        pub fn s16(Inject(p): Inject<S15>) -> Result<S16, InstantiateErrorKind> {
            Ok(S16(p))
        }
        pub fn t16(InjectTransient(p): InjectTransient<T15>) -> Result<T16, InstantiateErrorKind> {
            Ok(T16(Box::new(p)))
        }
        pub fn s17(Inject(p): Inject<S16>) -> Result<S17, InstantiateErrorKind> {
            Ok(S17(p))
        }
        pub fn t17(InjectTransient(p): InjectTransient<T16>) -> Result<T17, InstantiateErrorKind> {
            Ok(T17(Box::new(p)))
        }
        pub fn s18(Inject(p): Inject<S17>) -> Result<S18, InstantiateErrorKind> {
            Ok(S18(p))
        }
        pub fn t18(InjectTransient(p): InjectTransient<T17>) -> Result<T18, InstantiateErrorKind> {
            Ok(T18(Box::new(p)))
        }
        pub fn s19(Inject(p): Inject<S18>) -> Result<S19, InstantiateErrorKind> {
            Ok(S19(p))
        }
        pub fn t19(InjectTransient(p): InjectTransient<T18>) -> Result<T19, InstantiateErrorKind> {
            Ok(T19(Box::new(p)))
        }
        pub fn s20(Inject(p): Inject<S19>) -> Result<S20, InstantiateErrorKind> {
            Ok(S20(p))
        }
        pub fn t20(InjectTransient(p): InjectTransient<T19>) -> Result<T20, InstantiateErrorKind> {
            Ok(T20(Box::new(p)))
        }
        pub fn s21(Inject(p): Inject<S20>) -> Result<S21, InstantiateErrorKind> {
            Ok(S21(p))
        }
        pub fn t21(InjectTransient(p): InjectTransient<T20>) -> Result<T21, InstantiateErrorKind> {
            Ok(T21(Box::new(p)))
        }
        pub fn s22(Inject(p): Inject<S21>) -> Result<S22, InstantiateErrorKind> {
            Ok(S22(p))
        }
        pub fn t22(InjectTransient(p): InjectTransient<T21>) -> Result<T22, InstantiateErrorKind> {
            Ok(T22(Box::new(p)))
        }
        pub fn s23(Inject(p): Inject<S22>) -> Result<S23, InstantiateErrorKind> {
            Ok(S23(p))
        }
        pub fn t23(InjectTransient(p): InjectTransient<T22>) -> Result<T23, InstantiateErrorKind> {
            Ok(T23(Box::new(p)))
        }
        pub fn s24(Inject(p): Inject<S23>) -> Result<S24, InstantiateErrorKind> {
            Ok(S24(p))
        }
        pub fn t24(InjectTransient(p): InjectTransient<T23>) -> Result<T24, InstantiateErrorKind> {
            Ok(T24(Box::new(p)))
        }
        pub fn s25(Inject(p): Inject<S24>) -> Result<S25, InstantiateErrorKind> {
            Ok(S25(p))
        }
        pub fn t25(InjectTransient(p): InjectTransient<T24>) -> Result<T25, InstantiateErrorKind> {
            Ok(T25(Box::new(p)))
        }
        pub fn s26(Inject(p): Inject<S25>) -> Result<S26, InstantiateErrorKind> {
            Ok(S26(p))
        }
        pub fn t26(InjectTransient(p): InjectTransient<T25>) -> Result<T26, InstantiateErrorKind> {
            Ok(T26(Box::new(p)))
        }
        pub fn s27(Inject(p): Inject<S26>) -> Result<S27, InstantiateErrorKind> {
            Ok(S27(p))
        }
        pub fn t27(InjectTransient(p): InjectTransient<T26>) -> Result<T27, InstantiateErrorKind> {
            Ok(T27(Box::new(p)))
        }
        pub fn s28(Inject(p): Inject<S27>) -> Result<S28, InstantiateErrorKind> {
            Ok(S28(p))
        }
        pub fn t28(InjectTransient(p): InjectTransient<T27>) -> Result<T28, InstantiateErrorKind> {
            Ok(T28(Box::new(p)))
        }
        pub fn s29(Inject(p): Inject<S28>) -> Result<S29, InstantiateErrorKind> {
            Ok(S29(p))
        }
        pub fn t29(InjectTransient(p): InjectTransient<T28>) -> Result<T29, InstantiateErrorKind> {
            Ok(T29(Box::new(p)))
        }
        pub fn s30(Inject(p): Inject<S29>) -> Result<S30, InstantiateErrorKind> {
            Ok(S30(p))
        }
        pub fn t30(InjectTransient(p): InjectTransient<T29>) -> Result<T30, InstantiateErrorKind> {
            Ok(T30(Box::new(p)))
        }
        pub fn s31(Inject(p): Inject<S30>) -> Result<S31, InstantiateErrorKind> {
            Ok(S31(p))
        }
        pub fn t31(InjectTransient(p): InjectTransient<T30>) -> Result<T31, InstantiateErrorKind> {
            Ok(T31(Box::new(p)))
        }
        pub fn s32(Inject(p): Inject<S31>) -> Result<S32, InstantiateErrorKind> {
            Ok(S32(p))
        }
        pub fn t32(InjectTransient(p): InjectTransient<T31>) -> Result<T32, InstantiateErrorKind> {
            Ok(T32(Box::new(p)))
        }
        pub fn s33(Inject(p): Inject<S32>) -> Result<S33, InstantiateErrorKind> {
            Ok(S33(p))
        }
        pub fn t33(InjectTransient(p): InjectTransient<T32>) -> Result<T33, InstantiateErrorKind> {
            Ok(T33(Box::new(p)))
        }
        pub fn s34(Inject(p): Inject<S33>) -> Result<S34, InstantiateErrorKind> {
            Ok(S34(p))
        }
        pub fn t34(InjectTransient(p): InjectTransient<T33>) -> Result<T34, InstantiateErrorKind> {
            Ok(T34(Box::new(p)))
        }
        pub fn s35(Inject(p): Inject<S34>) -> Result<S35, InstantiateErrorKind> {
            Ok(S35(p))
        }
        pub fn t35(InjectTransient(p): InjectTransient<T34>) -> Result<T35, InstantiateErrorKind> {
            Ok(T35(Box::new(p)))
        }
        pub fn s36(Inject(p): Inject<S35>) -> Result<S36, InstantiateErrorKind> {
            Ok(S36(p))
        }
        pub fn t36(InjectTransient(p): InjectTransient<T35>) -> Result<T36, InstantiateErrorKind> {
            Ok(T36(Box::new(p)))
        }
        pub fn s37(Inject(p): Inject<S36>) -> Result<S37, InstantiateErrorKind> {
            Ok(S37(p))
        }
        pub fn t37(InjectTransient(p): InjectTransient<T36>) -> Result<T37, InstantiateErrorKind> {
            Ok(T37(Box::new(p)))
        }
        pub fn s38(Inject(p): Inject<S37>) -> Result<S38, InstantiateErrorKind> {
            Ok(S38(p))
        }
        pub fn t38(InjectTransient(p): InjectTransient<T37>) -> Result<T38, InstantiateErrorKind> {
            Ok(T38(Box::new(p)))
        }
        pub fn s39(Inject(p): Inject<S38>) -> Result<S39, InstantiateErrorKind> {
            Ok(S39(p))
        }
        pub fn t39(InjectTransient(p): InjectTransient<T38>) -> Result<T39, InstantiateErrorKind> {
            Ok(T39(Box::new(p)))
        }
        pub fn s40(Inject(p): Inject<S39>) -> Result<S40, InstantiateErrorKind> {
            Ok(S40(p))
        }
        pub fn t40(InjectTransient(p): InjectTransient<T39>) -> Result<T40, InstantiateErrorKind> {
            Ok(T40(Box::new(p)))
        }
        pub fn s41(Inject(p): Inject<S40>) -> Result<S41, InstantiateErrorKind> {
            Ok(S41(p))
        }
        pub fn t41(InjectTransient(p): InjectTransient<T40>) -> Result<T41, InstantiateErrorKind> {
            Ok(T41(Box::new(p)))
        }
        pub fn s42(Inject(p): Inject<S41>) -> Result<S42, InstantiateErrorKind> {
            Ok(S42(p))
        }
        pub fn t42(InjectTransient(p): InjectTransient<T41>) -> Result<T42, InstantiateErrorKind> {
            Ok(T42(Box::new(p)))
        }
        pub fn s43(Inject(p): Inject<S42>) -> Result<S43, InstantiateErrorKind> {
            Ok(S43(p))
        }
        pub fn t43(InjectTransient(p): InjectTransient<T42>) -> Result<T43, InstantiateErrorKind> {
            Ok(T43(Box::new(p)))
        }
        pub fn s44(Inject(p): Inject<S43>) -> Result<S44, InstantiateErrorKind> {
            Ok(S44(p))
        }
        pub fn t44(InjectTransient(p): InjectTransient<T43>) -> Result<T44, InstantiateErrorKind> {
            Ok(T44(Box::new(p)))
        }
        pub fn s45(Inject(p): Inject<S44>) -> Result<S45, InstantiateErrorKind> {
            Ok(S45(p))
        }
        pub fn t45(InjectTransient(p): InjectTransient<T44>) -> Result<T45, InstantiateErrorKind> {
            Ok(T45(Box::new(p)))
        }
        pub fn s46(Inject(p): Inject<S45>) -> Result<S46, InstantiateErrorKind> {
            Ok(S46(p))
        }
        pub fn t46(InjectTransient(p): InjectTransient<T45>) -> Result<T46, InstantiateErrorKind> {
            Ok(T46(Box::new(p)))
        }
        pub fn s47(Inject(p): Inject<S46>) -> Result<S47, InstantiateErrorKind> {
            Ok(S47(p))
        }
        pub fn t47(InjectTransient(p): InjectTransient<T46>) -> Result<T47, InstantiateErrorKind> {
            Ok(T47(Box::new(p)))
        }
        pub fn s48(Inject(p): Inject<S47>) -> Result<S48, InstantiateErrorKind> {
            Ok(S48(p))
        }
        pub fn t48(InjectTransient(p): InjectTransient<T47>) -> Result<T48, InstantiateErrorKind> {
            Ok(T48(Box::new(p)))
        }
        pub fn s49(Inject(p): Inject<S48>) -> Result<S49, InstantiateErrorKind> {
            Ok(S49(p))
        }
        pub fn t49(InjectTransient(p): InjectTransient<T48>) -> Result<T49, InstantiateErrorKind> {
            Ok(T49(Box::new(p)))
        }
        pub fn s50(Inject(p): Inject<S49>) -> Result<S50, InstantiateErrorKind> {
            Ok(S50(p))
        }
        pub fn t50(InjectTransient(p): InjectTransient<T49>) -> Result<T50, InstantiateErrorKind> {
            Ok(T50(Box::new(p)))
        }
        pub fn s51(Inject(p): Inject<S50>) -> Result<S51, InstantiateErrorKind> {
            Ok(S51(p))
        }
        pub fn t51(InjectTransient(p): InjectTransient<T50>) -> Result<T51, InstantiateErrorKind> {
            Ok(T51(Box::new(p)))
        }
        pub fn s52(Inject(p): Inject<S51>) -> Result<S52, InstantiateErrorKind> {
            Ok(S52(p))
        }
        pub fn t52(InjectTransient(p): InjectTransient<T51>) -> Result<T52, InstantiateErrorKind> {
            Ok(T52(Box::new(p)))
        }
        pub fn s53(Inject(p): Inject<S52>) -> Result<S53, InstantiateErrorKind> {
            Ok(S53(p))
        }
        pub fn t53(InjectTransient(p): InjectTransient<T52>) -> Result<T53, InstantiateErrorKind> {
            Ok(T53(Box::new(p)))
        }
        pub fn s54(Inject(p): Inject<S53>) -> Result<S54, InstantiateErrorKind> {
            Ok(S54(p))
        }
        pub fn t54(InjectTransient(p): InjectTransient<T53>) -> Result<T54, InstantiateErrorKind> {
            Ok(T54(Box::new(p)))
        }
        pub fn s55(Inject(p): Inject<S54>) -> Result<S55, InstantiateErrorKind> {
            Ok(S55(p))
        }
        pub fn t55(InjectTransient(p): InjectTransient<T54>) -> Result<T55, InstantiateErrorKind> {
            Ok(T55(Box::new(p)))
        }
        pub fn s56(Inject(p): Inject<S55>) -> Result<S56, InstantiateErrorKind> {
            Ok(S56(p))
        }
        pub fn t56(InjectTransient(p): InjectTransient<T55>) -> Result<T56, InstantiateErrorKind> {
            Ok(T56(Box::new(p)))
        }
        pub fn s57(Inject(p): Inject<S56>) -> Result<S57, InstantiateErrorKind> {
            Ok(S57(p))
        }
        pub fn t57(InjectTransient(p): InjectTransient<T56>) -> Result<T57, InstantiateErrorKind> {
            Ok(T57(Box::new(p)))
        }
        pub fn s58(Inject(p): Inject<S57>) -> Result<S58, InstantiateErrorKind> {
            Ok(S58(p))
        }
        pub fn t58(InjectTransient(p): InjectTransient<T57>) -> Result<T58, InstantiateErrorKind> {
            Ok(T58(Box::new(p)))
        }
        pub fn s59(Inject(p): Inject<S58>) -> Result<S59, InstantiateErrorKind> {
            Ok(S59(p))
        }
        pub fn t59(InjectTransient(p): InjectTransient<T58>) -> Result<T59, InstantiateErrorKind> {
            Ok(T59(Box::new(p)))
        }
        pub fn s60(Inject(p): Inject<S59>) -> Result<S60, InstantiateErrorKind> {
            Ok(S60(p))
        }
        pub fn t60(InjectTransient(p): InjectTransient<T59>) -> Result<T60, InstantiateErrorKind> {
            Ok(T60(Box::new(p)))
        }
        pub fn s61(Inject(p): Inject<S60>) -> Result<S61, InstantiateErrorKind> {
            Ok(S61(p))
        }
        pub fn t61(InjectTransient(p): InjectTransient<T60>) -> Result<T61, InstantiateErrorKind> {
            Ok(T61(Box::new(p)))
        }
        pub fn s62(Inject(p): Inject<S61>) -> Result<S62, InstantiateErrorKind> {
            Ok(S62(p))
        }
        pub fn t62(InjectTransient(p): InjectTransient<T61>) -> Result<T62, InstantiateErrorKind> {
            Ok(T62(Box::new(p)))
        }
        pub fn s63(Inject(p): Inject<S62>) -> Result<S63, InstantiateErrorKind> {
            Ok(S63(p))
        }
        pub fn t63(InjectTransient(p): InjectTransient<T62>) -> Result<T63, InstantiateErrorKind> {
            Ok(T63(Box::new(p)))
        }
        pub fn s64(Inject(p): Inject<S63>) -> Result<S64, InstantiateErrorKind> {
            Ok(S64(p))
        }
        pub fn t64(InjectTransient(p): InjectTransient<T63>) -> Result<T64, InstantiateErrorKind> {
            Ok(T64(Box::new(p)))
        }
        pub fn s65(Inject(p): Inject<S64>) -> Result<S65, InstantiateErrorKind> {
            Ok(S65(p))
        }
        pub fn t65(InjectTransient(p): InjectTransient<T64>) -> Result<T65, InstantiateErrorKind> {
            Ok(T65(Box::new(p)))
        }
        pub fn s66(Inject(p): Inject<S65>) -> Result<S66, InstantiateErrorKind> {
            Ok(S66(p))
        }
        pub fn t66(InjectTransient(p): InjectTransient<T65>) -> Result<T66, InstantiateErrorKind> {
            Ok(T66(Box::new(p)))
        }
        pub fn s67(Inject(p): Inject<S66>) -> Result<S67, InstantiateErrorKind> {
            Ok(S67(p))
        }
        pub fn t67(InjectTransient(p): InjectTransient<T66>) -> Result<T67, InstantiateErrorKind> {
            Ok(T67(Box::new(p)))
        }
        pub fn s68(Inject(p): Inject<S67>) -> Result<S68, InstantiateErrorKind> {
            Ok(S68(p))
        }
        pub fn t68(InjectTransient(p): InjectTransient<T67>) -> Result<T68, InstantiateErrorKind> {
            Ok(T68(Box::new(p)))
        }
        pub fn s69(Inject(p): Inject<S68>) -> Result<S69, InstantiateErrorKind> {
            Ok(S69(p))
        }
        pub fn t69(InjectTransient(p): InjectTransient<T68>) -> Result<T69, InstantiateErrorKind> {
            Ok(T69(Box::new(p)))
        }
        pub fn s70(Inject(p): Inject<S69>) -> Result<S70, InstantiateErrorKind> {
            Ok(S70(p))
        }
        pub fn t70(InjectTransient(p): InjectTransient<T69>) -> Result<T70, InstantiateErrorKind> {
            Ok(T70(Box::new(p)))
        }
        pub fn s71(Inject(p): Inject<S70>) -> Result<S71, InstantiateErrorKind> {
            Ok(S71(p))
        }
        pub fn t71(InjectTransient(p): InjectTransient<T70>) -> Result<T71, InstantiateErrorKind> {
            Ok(T71(Box::new(p)))
        }
        pub fn s72(Inject(p): Inject<S71>) -> Result<S72, InstantiateErrorKind> {
            Ok(S72(p))
        }
        pub fn t72(InjectTransient(p): InjectTransient<T71>) -> Result<T72, InstantiateErrorKind> {
            Ok(T72(Box::new(p)))
        }
        pub fn s73(Inject(p): Inject<S72>) -> Result<S73, InstantiateErrorKind> {
            Ok(S73(p))
        }
        pub fn t73(InjectTransient(p): InjectTransient<T72>) -> Result<T73, InstantiateErrorKind> {
            Ok(T73(Box::new(p)))
        }
        pub fn s74(Inject(p): Inject<S73>) -> Result<S74, InstantiateErrorKind> {
            Ok(S74(p))
        }
        pub fn t74(InjectTransient(p): InjectTransient<T73>) -> Result<T74, InstantiateErrorKind> {
            Ok(T74(Box::new(p)))
        }
        pub fn s75(Inject(p): Inject<S74>) -> Result<S75, InstantiateErrorKind> {
            Ok(S75(p))
        }
        pub fn t75(InjectTransient(p): InjectTransient<T74>) -> Result<T75, InstantiateErrorKind> {
            Ok(T75(Box::new(p)))
        }
        pub fn s76(Inject(p): Inject<S75>) -> Result<S76, InstantiateErrorKind> {
            Ok(S76(p))
        }
        pub fn t76(InjectTransient(p): InjectTransient<T75>) -> Result<T76, InstantiateErrorKind> {
            Ok(T76(Box::new(p)))
        }
        pub fn s77(Inject(p): Inject<S76>) -> Result<S77, InstantiateErrorKind> {
            Ok(S77(p))
        }
        pub fn t77(InjectTransient(p): InjectTransient<T76>) -> Result<T77, InstantiateErrorKind> {
            Ok(T77(Box::new(p)))
        }
        pub fn s78(Inject(p): Inject<S77>) -> Result<S78, InstantiateErrorKind> {
            Ok(S78(p))
        }
        pub fn t78(InjectTransient(p): InjectTransient<T77>) -> Result<T78, InstantiateErrorKind> {
            Ok(T78(Box::new(p)))
        }
        pub fn s79(Inject(p): Inject<S78>) -> Result<S79, InstantiateErrorKind> {
            Ok(S79(p))
        }
        pub fn t79(InjectTransient(p): InjectTransient<T78>) -> Result<T79, InstantiateErrorKind> {
            Ok(T79(Box::new(p)))
        }
        pub fn s80(Inject(p): Inject<S79>) -> Result<S80, InstantiateErrorKind> {
            Ok(S80(p))
        }
        pub fn t80(InjectTransient(p): InjectTransient<T79>) -> Result<T80, InstantiateErrorKind> {
            Ok(T80(Box::new(p)))
        }
        pub fn s81(Inject(p): Inject<S80>) -> Result<S81, InstantiateErrorKind> {
            Ok(S81(p))
        }
        pub fn t81(InjectTransient(p): InjectTransient<T80>) -> Result<T81, InstantiateErrorKind> {
            Ok(T81(Box::new(p)))
        }
        pub fn s82(Inject(p): Inject<S81>) -> Result<S82, InstantiateErrorKind> {
            Ok(S82(p))
        }
        pub fn t82(InjectTransient(p): InjectTransient<T81>) -> Result<T82, InstantiateErrorKind> {
            Ok(T82(Box::new(p)))
        }
        pub fn s83(Inject(p): Inject<S82>) -> Result<S83, InstantiateErrorKind> {
            Ok(S83(p))
        }
        pub fn t83(InjectTransient(p): InjectTransient<T82>) -> Result<T83, InstantiateErrorKind> {
            Ok(T83(Box::new(p)))
        }
        pub fn s84(Inject(p): Inject<S83>) -> Result<S84, InstantiateErrorKind> {
            Ok(S84(p))
        }
        pub fn t84(InjectTransient(p): InjectTransient<T83>) -> Result<T84, InstantiateErrorKind> {
            Ok(T84(Box::new(p)))
        }
        pub fn s85(Inject(p): Inject<S84>) -> Result<S85, InstantiateErrorKind> {
            Ok(S85(p))
        }
        pub fn t85(InjectTransient(p): InjectTransient<T84>) -> Result<T85, InstantiateErrorKind> {
            Ok(T85(Box::new(p)))
        }
        pub fn s86(Inject(p): Inject<S85>) -> Result<S86, InstantiateErrorKind> {
            Ok(S86(p))
        }
        pub fn t86(InjectTransient(p): InjectTransient<T85>) -> Result<T86, InstantiateErrorKind> {
            Ok(T86(Box::new(p)))
        }
        pub fn s87(Inject(p): Inject<S86>) -> Result<S87, InstantiateErrorKind> {
            Ok(S87(p))
        }
        pub fn t87(InjectTransient(p): InjectTransient<T86>) -> Result<T87, InstantiateErrorKind> {
            Ok(T87(Box::new(p)))
        }
        pub fn s88(Inject(p): Inject<S87>) -> Result<S88, InstantiateErrorKind> {
            Ok(S88(p))
        }
        pub fn t88(InjectTransient(p): InjectTransient<T87>) -> Result<T88, InstantiateErrorKind> {
            Ok(T88(Box::new(p)))
        }
        pub fn s89(Inject(p): Inject<S88>) -> Result<S89, InstantiateErrorKind> {
            Ok(S89(p))
        }
        pub fn t89(InjectTransient(p): InjectTransient<T88>) -> Result<T89, InstantiateErrorKind> {
            Ok(T89(Box::new(p)))
        }
        pub fn s90(Inject(p): Inject<S89>) -> Result<S90, InstantiateErrorKind> {
            Ok(S90(p))
        }
        pub fn t90(InjectTransient(p): InjectTransient<T89>) -> Result<T90, InstantiateErrorKind> {
            Ok(T90(Box::new(p)))
        }
        pub fn s91(Inject(p): Inject<S90>) -> Result<S91, InstantiateErrorKind> {
            Ok(S91(p))
        }
        pub fn t91(InjectTransient(p): InjectTransient<T90>) -> Result<T91, InstantiateErrorKind> {
            Ok(T91(Box::new(p)))
        }
        pub fn s92(Inject(p): Inject<S91>) -> Result<S92, InstantiateErrorKind> {
            Ok(S92(p))
        }
        pub fn t92(InjectTransient(p): InjectTransient<T91>) -> Result<T92, InstantiateErrorKind> {
            Ok(T92(Box::new(p)))
        }
        pub fn s93(Inject(p): Inject<S92>) -> Result<S93, InstantiateErrorKind> {
            Ok(S93(p))
        }
        pub fn t93(InjectTransient(p): InjectTransient<T92>) -> Result<T93, InstantiateErrorKind> {
            Ok(T93(Box::new(p)))
        }
        pub fn s94(Inject(p): Inject<S93>) -> Result<S94, InstantiateErrorKind> {
            Ok(S94(p))
        }
        pub fn t94(InjectTransient(p): InjectTransient<T93>) -> Result<T94, InstantiateErrorKind> {
            Ok(T94(Box::new(p)))
        }
        pub fn s95(Inject(p): Inject<S94>) -> Result<S95, InstantiateErrorKind> {
            Ok(S95(p))
        }
        pub fn t95(InjectTransient(p): InjectTransient<T94>) -> Result<T95, InstantiateErrorKind> {
            Ok(T95(Box::new(p)))
        }
        pub fn s96(Inject(p): Inject<S95>) -> Result<S96, InstantiateErrorKind> {
            Ok(S96(p))
        }
        pub fn t96(InjectTransient(p): InjectTransient<T95>) -> Result<T96, InstantiateErrorKind> {
            Ok(T96(Box::new(p)))
        }
        pub fn s97(Inject(p): Inject<S96>) -> Result<S97, InstantiateErrorKind> {
            Ok(S97(p))
        }
        pub fn t97(InjectTransient(p): InjectTransient<T96>) -> Result<T97, InstantiateErrorKind> {
            Ok(T97(Box::new(p)))
        }
        pub fn s98(Inject(p): Inject<S97>) -> Result<S98, InstantiateErrorKind> {
            Ok(S98(p))
        }
        pub fn t98(InjectTransient(p): InjectTransient<T97>) -> Result<T98, InstantiateErrorKind> {
            Ok(T98(Box::new(p)))
        }
        pub fn s99(Inject(p): Inject<S98>) -> Result<S99, InstantiateErrorKind> {
            Ok(S99(p))
        }
        pub fn t99(InjectTransient(p): InjectTransient<T98>) -> Result<T99, InstantiateErrorKind> {
            Ok(T99(Box::new(p)))
        }
        pub fn wide(
            Inject(_0): Inject<S0>,
            Inject(_1): Inject<S0>,
            Inject(_2): Inject<S0>,
            Inject(_3): Inject<S0>,
            Inject(_4): Inject<S0>,
            Inject(_5): Inject<S0>,
            Inject(_6): Inject<S0>,
            Inject(_7): Inject<S0>,
            Inject(_8): Inject<S0>,
            Inject(_9): Inject<S0>,
            Inject(_10): Inject<S0>,
            Inject(_11): Inject<S0>,
            Inject(_12): Inject<S0>,
            Inject(_13): Inject<S0>,
            Inject(_14): Inject<S0>,
            Inject(_15): Inject<S0>,
        ) -> Result<Wide, InstantiateErrorKind> {
            Ok(Wide([0; 16]))
        }
        pub fn host(Inject(p): Inject<Plugin>) -> Result<Host, InstantiateErrorKind> {
            Ok(Host(p))
        }
    }

    pub fn chain(app: bool) -> Container {
        if app {
            Container::new(
                registry! { scope(App) [ provide(f::s0), provide(f::s1), provide(f::s2), provide(f::s3), provide(f::s4), provide(f::s5), provide(f::s6), provide(f::s7), provide(f::s8), provide(f::s9), provide(f::s10), provide(f::s11), provide(f::s12), provide(f::s13), provide(f::s14), provide(f::s15), provide(f::s16), provide(f::s17), provide(f::s18), provide(f::s19), provide(f::s20), provide(f::s21), provide(f::s22), provide(f::s23), provide(f::s24), provide(f::s25), provide(f::s26), provide(f::s27), provide(f::s28), provide(f::s29), provide(f::s30), provide(f::s31), provide(f::s32), provide(f::s33), provide(f::s34), provide(f::s35), provide(f::s36), provide(f::s37), provide(f::s38), provide(f::s39), provide(f::s40), provide(f::s41), provide(f::s42), provide(f::s43), provide(f::s44), provide(f::s45), provide(f::s46), provide(f::s47), provide(f::s48), provide(f::s49), provide(f::s50), provide(f::s51), provide(f::s52), provide(f::s53), provide(f::s54), provide(f::s55), provide(f::s56), provide(f::s57), provide(f::s58), provide(f::s59), provide(f::s60), provide(f::s61), provide(f::s62), provide(f::s63), provide(f::s64), provide(f::s65), provide(f::s66), provide(f::s67), provide(f::s68), provide(f::s69), provide(f::s70), provide(f::s71), provide(f::s72), provide(f::s73), provide(f::s74), provide(f::s75), provide(f::s76), provide(f::s77), provide(f::s78), provide(f::s79), provide(f::s80), provide(f::s81), provide(f::s82), provide(f::s83), provide(f::s84), provide(f::s85), provide(f::s86), provide(f::s87), provide(f::s88), provide(f::s89), provide(f::s90), provide(f::s91), provide(f::s92), provide(f::s93), provide(f::s94), provide(f::s95), provide(f::s96), provide(f::s97), provide(f::s98), provide(f::s99) ] },
            )
        } else {
            Container::new(
                registry! { scope(Request) [ provide(f::s0), provide(f::s1), provide(f::s2), provide(f::s3), provide(f::s4), provide(f::s5), provide(f::s6), provide(f::s7), provide(f::s8), provide(f::s9), provide(f::s10), provide(f::s11), provide(f::s12), provide(f::s13), provide(f::s14), provide(f::s15), provide(f::s16), provide(f::s17), provide(f::s18), provide(f::s19), provide(f::s20), provide(f::s21), provide(f::s22), provide(f::s23), provide(f::s24), provide(f::s25), provide(f::s26), provide(f::s27), provide(f::s28), provide(f::s29), provide(f::s30), provide(f::s31), provide(f::s32), provide(f::s33), provide(f::s34), provide(f::s35), provide(f::s36), provide(f::s37), provide(f::s38), provide(f::s39), provide(f::s40), provide(f::s41), provide(f::s42), provide(f::s43), provide(f::s44), provide(f::s45), provide(f::s46), provide(f::s47), provide(f::s48), provide(f::s49), provide(f::s50), provide(f::s51), provide(f::s52), provide(f::s53), provide(f::s54), provide(f::s55), provide(f::s56), provide(f::s57), provide(f::s58), provide(f::s59), provide(f::s60), provide(f::s61), provide(f::s62), provide(f::s63), provide(f::s64), provide(f::s65), provide(f::s66), provide(f::s67), provide(f::s68), provide(f::s69), provide(f::s70), provide(f::s71), provide(f::s72), provide(f::s73), provide(f::s74), provide(f::s75), provide(f::s76), provide(f::s77), provide(f::s78), provide(f::s79), provide(f::s80), provide(f::s81), provide(f::s82), provide(f::s83), provide(f::s84), provide(f::s85), provide(f::s86), provide(f::s87), provide(f::s88), provide(f::s89), provide(f::s90), provide(f::s91), provide(f::s92), provide(f::s93), provide(f::s94), provide(f::s95), provide(f::s96), provide(f::s97), provide(f::s98), provide(f::s99) ] },
            )
        }
    }

    pub fn transient_chain() -> Container {
        Container::new(
            registry! { scope(App) [ provide(f::t0), provide(f::t1), provide(f::t2), provide(f::t3), provide(f::t4), provide(f::t5), provide(f::t6), provide(f::t7), provide(f::t8), provide(f::t9), provide(f::t10), provide(f::t11), provide(f::t12), provide(f::t13), provide(f::t14), provide(f::t15), provide(f::t16), provide(f::t17), provide(f::t18), provide(f::t19), provide(f::t20), provide(f::t21), provide(f::t22), provide(f::t23), provide(f::t24), provide(f::t25), provide(f::t26), provide(f::t27), provide(f::t28), provide(f::t29), provide(f::t30), provide(f::t31), provide(f::t32), provide(f::t33), provide(f::t34), provide(f::t35), provide(f::t36), provide(f::t37), provide(f::t38), provide(f::t39), provide(f::t40), provide(f::t41), provide(f::t42), provide(f::t43), provide(f::t44), provide(f::t45), provide(f::t46), provide(f::t47), provide(f::t48), provide(f::t49), provide(f::t50), provide(f::t51), provide(f::t52), provide(f::t53), provide(f::t54), provide(f::t55), provide(f::t56), provide(f::t57), provide(f::t58), provide(f::t59), provide(f::t60), provide(f::t61), provide(f::t62), provide(f::t63), provide(f::t64), provide(f::t65), provide(f::t66), provide(f::t67), provide(f::t68), provide(f::t69), provide(f::t70), provide(f::t71), provide(f::t72), provide(f::t73), provide(f::t74), provide(f::t75), provide(f::t76), provide(f::t77), provide(f::t78), provide(f::t79), provide(f::t80), provide(f::t81), provide(f::t82), provide(f::t83), provide(f::t84), provide(f::t85), provide(f::t86), provide(f::t87), provide(f::t88), provide(f::t89), provide(f::t90), provide(f::t91), provide(f::t92), provide(f::t93), provide(f::t94), provide(f::t95), provide(f::t96), provide(f::t97), provide(f::t98), provide(f::t99) ] },
        )
    }

    pub fn wide() -> Container {
        Container::new(registry! { scope(App) [ provide(f::s0), provide(f::wide) ] })
    }

    pub fn captured() -> Container {
        let label = Arc::new(String::from("captured"));
        Container::new(registry! { scope(Request) [ provide(move || Ok::<_, InstantiateErrorKind>(label.clone())) ] })
    }
}

pub mod compile_engine {
    use super::*;
    use froodi_compile::{registry, Container, DefaultScope::*, Inject, InjectTransient, InstantiateErrorKind};

    mod f {
        use super::*;
        pub fn s0() -> Result<S0, InstantiateErrorKind> {
            Ok(S0)
        }
        pub fn t0() -> Result<T0, InstantiateErrorKind> {
            Ok(T0)
        }
        pub fn s1(Inject(p): Inject<S0>) -> Result<S1, InstantiateErrorKind> {
            Ok(S1(p))
        }
        pub fn t1(InjectTransient(p): InjectTransient<T0>) -> Result<T1, InstantiateErrorKind> {
            Ok(T1(Box::new(p)))
        }
        pub fn s2(Inject(p): Inject<S1>) -> Result<S2, InstantiateErrorKind> {
            Ok(S2(p))
        }
        pub fn t2(InjectTransient(p): InjectTransient<T1>) -> Result<T2, InstantiateErrorKind> {
            Ok(T2(Box::new(p)))
        }
        pub fn s3(Inject(p): Inject<S2>) -> Result<S3, InstantiateErrorKind> {
            Ok(S3(p))
        }
        pub fn t3(InjectTransient(p): InjectTransient<T2>) -> Result<T3, InstantiateErrorKind> {
            Ok(T3(Box::new(p)))
        }
        pub fn s4(Inject(p): Inject<S3>) -> Result<S4, InstantiateErrorKind> {
            Ok(S4(p))
        }
        pub fn t4(InjectTransient(p): InjectTransient<T3>) -> Result<T4, InstantiateErrorKind> {
            Ok(T4(Box::new(p)))
        }
        pub fn s5(Inject(p): Inject<S4>) -> Result<S5, InstantiateErrorKind> {
            Ok(S5(p))
        }
        pub fn t5(InjectTransient(p): InjectTransient<T4>) -> Result<T5, InstantiateErrorKind> {
            Ok(T5(Box::new(p)))
        }
        pub fn s6(Inject(p): Inject<S5>) -> Result<S6, InstantiateErrorKind> {
            Ok(S6(p))
        }
        pub fn t6(InjectTransient(p): InjectTransient<T5>) -> Result<T6, InstantiateErrorKind> {
            Ok(T6(Box::new(p)))
        }
        pub fn s7(Inject(p): Inject<S6>) -> Result<S7, InstantiateErrorKind> {
            Ok(S7(p))
        }
        pub fn t7(InjectTransient(p): InjectTransient<T6>) -> Result<T7, InstantiateErrorKind> {
            Ok(T7(Box::new(p)))
        }
        pub fn s8(Inject(p): Inject<S7>) -> Result<S8, InstantiateErrorKind> {
            Ok(S8(p))
        }
        pub fn t8(InjectTransient(p): InjectTransient<T7>) -> Result<T8, InstantiateErrorKind> {
            Ok(T8(Box::new(p)))
        }
        pub fn s9(Inject(p): Inject<S8>) -> Result<S9, InstantiateErrorKind> {
            Ok(S9(p))
        }
        pub fn t9(InjectTransient(p): InjectTransient<T8>) -> Result<T9, InstantiateErrorKind> {
            Ok(T9(Box::new(p)))
        }
        pub fn s10(Inject(p): Inject<S9>) -> Result<S10, InstantiateErrorKind> {
            Ok(S10(p))
        }
        pub fn t10(InjectTransient(p): InjectTransient<T9>) -> Result<T10, InstantiateErrorKind> {
            Ok(T10(Box::new(p)))
        }
        pub fn s11(Inject(p): Inject<S10>) -> Result<S11, InstantiateErrorKind> {
            Ok(S11(p))
        }
        pub fn t11(InjectTransient(p): InjectTransient<T10>) -> Result<T11, InstantiateErrorKind> {
            Ok(T11(Box::new(p)))
        }
        pub fn s12(Inject(p): Inject<S11>) -> Result<S12, InstantiateErrorKind> {
            Ok(S12(p))
        }
        pub fn t12(InjectTransient(p): InjectTransient<T11>) -> Result<T12, InstantiateErrorKind> {
            Ok(T12(Box::new(p)))
        }
        pub fn s13(Inject(p): Inject<S12>) -> Result<S13, InstantiateErrorKind> {
            Ok(S13(p))
        }
        pub fn t13(InjectTransient(p): InjectTransient<T12>) -> Result<T13, InstantiateErrorKind> {
            Ok(T13(Box::new(p)))
        }
        pub fn s14(Inject(p): Inject<S13>) -> Result<S14, InstantiateErrorKind> {
            Ok(S14(p))
        }
        pub fn t14(InjectTransient(p): InjectTransient<T13>) -> Result<T14, InstantiateErrorKind> {
            Ok(T14(Box::new(p)))
        }
        pub fn s15(Inject(p): Inject<S14>) -> Result<S15, InstantiateErrorKind> {
            Ok(S15(p))
        }
        pub fn t15(InjectTransient(p): InjectTransient<T14>) -> Result<T15, InstantiateErrorKind> {
            Ok(T15(Box::new(p)))
        }
        pub fn s16(Inject(p): Inject<S15>) -> Result<S16, InstantiateErrorKind> {
            Ok(S16(p))
        }
        pub fn t16(InjectTransient(p): InjectTransient<T15>) -> Result<T16, InstantiateErrorKind> {
            Ok(T16(Box::new(p)))
        }
        pub fn s17(Inject(p): Inject<S16>) -> Result<S17, InstantiateErrorKind> {
            Ok(S17(p))
        }
        pub fn t17(InjectTransient(p): InjectTransient<T16>) -> Result<T17, InstantiateErrorKind> {
            Ok(T17(Box::new(p)))
        }
        pub fn s18(Inject(p): Inject<S17>) -> Result<S18, InstantiateErrorKind> {
            Ok(S18(p))
        }
        pub fn t18(InjectTransient(p): InjectTransient<T17>) -> Result<T18, InstantiateErrorKind> {
            Ok(T18(Box::new(p)))
        }
        pub fn s19(Inject(p): Inject<S18>) -> Result<S19, InstantiateErrorKind> {
            Ok(S19(p))
        }
        pub fn t19(InjectTransient(p): InjectTransient<T18>) -> Result<T19, InstantiateErrorKind> {
            Ok(T19(Box::new(p)))
        }
        pub fn s20(Inject(p): Inject<S19>) -> Result<S20, InstantiateErrorKind> {
            Ok(S20(p))
        }
        pub fn t20(InjectTransient(p): InjectTransient<T19>) -> Result<T20, InstantiateErrorKind> {
            Ok(T20(Box::new(p)))
        }
        pub fn s21(Inject(p): Inject<S20>) -> Result<S21, InstantiateErrorKind> {
            Ok(S21(p))
        }
        pub fn t21(InjectTransient(p): InjectTransient<T20>) -> Result<T21, InstantiateErrorKind> {
            Ok(T21(Box::new(p)))
        }
        pub fn s22(Inject(p): Inject<S21>) -> Result<S22, InstantiateErrorKind> {
            Ok(S22(p))
        }
        pub fn t22(InjectTransient(p): InjectTransient<T21>) -> Result<T22, InstantiateErrorKind> {
            Ok(T22(Box::new(p)))
        }
        pub fn s23(Inject(p): Inject<S22>) -> Result<S23, InstantiateErrorKind> {
            Ok(S23(p))
        }
        pub fn t23(InjectTransient(p): InjectTransient<T22>) -> Result<T23, InstantiateErrorKind> {
            Ok(T23(Box::new(p)))
        }
        pub fn s24(Inject(p): Inject<S23>) -> Result<S24, InstantiateErrorKind> {
            Ok(S24(p))
        }
        pub fn t24(InjectTransient(p): InjectTransient<T23>) -> Result<T24, InstantiateErrorKind> {
            Ok(T24(Box::new(p)))
        }
        pub fn s25(Inject(p): Inject<S24>) -> Result<S25, InstantiateErrorKind> {
            Ok(S25(p))
        }
        pub fn t25(InjectTransient(p): InjectTransient<T24>) -> Result<T25, InstantiateErrorKind> {
            Ok(T25(Box::new(p)))
        }
        pub fn s26(Inject(p): Inject<S25>) -> Result<S26, InstantiateErrorKind> {
            Ok(S26(p))
        }
        pub fn t26(InjectTransient(p): InjectTransient<T25>) -> Result<T26, InstantiateErrorKind> {
            Ok(T26(Box::new(p)))
        }
        pub fn s27(Inject(p): Inject<S26>) -> Result<S27, InstantiateErrorKind> {
            Ok(S27(p))
        }
        pub fn t27(InjectTransient(p): InjectTransient<T26>) -> Result<T27, InstantiateErrorKind> {
            Ok(T27(Box::new(p)))
        }
        pub fn s28(Inject(p): Inject<S27>) -> Result<S28, InstantiateErrorKind> {
            Ok(S28(p))
        }
        pub fn t28(InjectTransient(p): InjectTransient<T27>) -> Result<T28, InstantiateErrorKind> {
            Ok(T28(Box::new(p)))
        }
        pub fn s29(Inject(p): Inject<S28>) -> Result<S29, InstantiateErrorKind> {
            Ok(S29(p))
        }
        pub fn t29(InjectTransient(p): InjectTransient<T28>) -> Result<T29, InstantiateErrorKind> {
            Ok(T29(Box::new(p)))
        }
        pub fn s30(Inject(p): Inject<S29>) -> Result<S30, InstantiateErrorKind> {
            Ok(S30(p))
        }
        pub fn t30(InjectTransient(p): InjectTransient<T29>) -> Result<T30, InstantiateErrorKind> {
            Ok(T30(Box::new(p)))
        }
        pub fn s31(Inject(p): Inject<S30>) -> Result<S31, InstantiateErrorKind> {
            Ok(S31(p))
        }
        pub fn t31(InjectTransient(p): InjectTransient<T30>) -> Result<T31, InstantiateErrorKind> {
            Ok(T31(Box::new(p)))
        }
        pub fn s32(Inject(p): Inject<S31>) -> Result<S32, InstantiateErrorKind> {
            Ok(S32(p))
        }
        pub fn t32(InjectTransient(p): InjectTransient<T31>) -> Result<T32, InstantiateErrorKind> {
            Ok(T32(Box::new(p)))
        }
        pub fn s33(Inject(p): Inject<S32>) -> Result<S33, InstantiateErrorKind> {
            Ok(S33(p))
        }
        pub fn t33(InjectTransient(p): InjectTransient<T32>) -> Result<T33, InstantiateErrorKind> {
            Ok(T33(Box::new(p)))
        }
        pub fn s34(Inject(p): Inject<S33>) -> Result<S34, InstantiateErrorKind> {
            Ok(S34(p))
        }
        pub fn t34(InjectTransient(p): InjectTransient<T33>) -> Result<T34, InstantiateErrorKind> {
            Ok(T34(Box::new(p)))
        }
        pub fn s35(Inject(p): Inject<S34>) -> Result<S35, InstantiateErrorKind> {
            Ok(S35(p))
        }
        pub fn t35(InjectTransient(p): InjectTransient<T34>) -> Result<T35, InstantiateErrorKind> {
            Ok(T35(Box::new(p)))
        }
        pub fn s36(Inject(p): Inject<S35>) -> Result<S36, InstantiateErrorKind> {
            Ok(S36(p))
        }
        pub fn t36(InjectTransient(p): InjectTransient<T35>) -> Result<T36, InstantiateErrorKind> {
            Ok(T36(Box::new(p)))
        }
        pub fn s37(Inject(p): Inject<S36>) -> Result<S37, InstantiateErrorKind> {
            Ok(S37(p))
        }
        pub fn t37(InjectTransient(p): InjectTransient<T36>) -> Result<T37, InstantiateErrorKind> {
            Ok(T37(Box::new(p)))
        }
        pub fn s38(Inject(p): Inject<S37>) -> Result<S38, InstantiateErrorKind> {
            Ok(S38(p))
        }
        pub fn t38(InjectTransient(p): InjectTransient<T37>) -> Result<T38, InstantiateErrorKind> {
            Ok(T38(Box::new(p)))
        }
        pub fn s39(Inject(p): Inject<S38>) -> Result<S39, InstantiateErrorKind> {
            Ok(S39(p))
        }
        pub fn t39(InjectTransient(p): InjectTransient<T38>) -> Result<T39, InstantiateErrorKind> {
            Ok(T39(Box::new(p)))
        }
        pub fn s40(Inject(p): Inject<S39>) -> Result<S40, InstantiateErrorKind> {
            Ok(S40(p))
        }
        pub fn t40(InjectTransient(p): InjectTransient<T39>) -> Result<T40, InstantiateErrorKind> {
            Ok(T40(Box::new(p)))
        }
        pub fn s41(Inject(p): Inject<S40>) -> Result<S41, InstantiateErrorKind> {
            Ok(S41(p))
        }
        pub fn t41(InjectTransient(p): InjectTransient<T40>) -> Result<T41, InstantiateErrorKind> {
            Ok(T41(Box::new(p)))
        }
        pub fn s42(Inject(p): Inject<S41>) -> Result<S42, InstantiateErrorKind> {
            Ok(S42(p))
        }
        pub fn t42(InjectTransient(p): InjectTransient<T41>) -> Result<T42, InstantiateErrorKind> {
            Ok(T42(Box::new(p)))
        }
        pub fn s43(Inject(p): Inject<S42>) -> Result<S43, InstantiateErrorKind> {
            Ok(S43(p))
        }
        pub fn t43(InjectTransient(p): InjectTransient<T42>) -> Result<T43, InstantiateErrorKind> {
            Ok(T43(Box::new(p)))
        }
        pub fn s44(Inject(p): Inject<S43>) -> Result<S44, InstantiateErrorKind> {
            Ok(S44(p))
        }
        pub fn t44(InjectTransient(p): InjectTransient<T43>) -> Result<T44, InstantiateErrorKind> {
            Ok(T44(Box::new(p)))
        }
        pub fn s45(Inject(p): Inject<S44>) -> Result<S45, InstantiateErrorKind> {
            Ok(S45(p))
        }
        pub fn t45(InjectTransient(p): InjectTransient<T44>) -> Result<T45, InstantiateErrorKind> {
            Ok(T45(Box::new(p)))
        }
        pub fn s46(Inject(p): Inject<S45>) -> Result<S46, InstantiateErrorKind> {
            Ok(S46(p))
        }
        pub fn t46(InjectTransient(p): InjectTransient<T45>) -> Result<T46, InstantiateErrorKind> {
            Ok(T46(Box::new(p)))
        }
        pub fn s47(Inject(p): Inject<S46>) -> Result<S47, InstantiateErrorKind> {
            Ok(S47(p))
        }
        pub fn t47(InjectTransient(p): InjectTransient<T46>) -> Result<T47, InstantiateErrorKind> {
            Ok(T47(Box::new(p)))
        }
        pub fn s48(Inject(p): Inject<S47>) -> Result<S48, InstantiateErrorKind> {
            Ok(S48(p))
        }
        pub fn t48(InjectTransient(p): InjectTransient<T47>) -> Result<T48, InstantiateErrorKind> {
            Ok(T48(Box::new(p)))
        }
        pub fn s49(Inject(p): Inject<S48>) -> Result<S49, InstantiateErrorKind> {
            Ok(S49(p))
        }
        pub fn t49(InjectTransient(p): InjectTransient<T48>) -> Result<T49, InstantiateErrorKind> {
            Ok(T49(Box::new(p)))
        }
        pub fn s50(Inject(p): Inject<S49>) -> Result<S50, InstantiateErrorKind> {
            Ok(S50(p))
        }
        pub fn t50(InjectTransient(p): InjectTransient<T49>) -> Result<T50, InstantiateErrorKind> {
            Ok(T50(Box::new(p)))
        }
        pub fn s51(Inject(p): Inject<S50>) -> Result<S51, InstantiateErrorKind> {
            Ok(S51(p))
        }
        pub fn t51(InjectTransient(p): InjectTransient<T50>) -> Result<T51, InstantiateErrorKind> {
            Ok(T51(Box::new(p)))
        }
        pub fn s52(Inject(p): Inject<S51>) -> Result<S52, InstantiateErrorKind> {
            Ok(S52(p))
        }
        pub fn t52(InjectTransient(p): InjectTransient<T51>) -> Result<T52, InstantiateErrorKind> {
            Ok(T52(Box::new(p)))
        }
        pub fn s53(Inject(p): Inject<S52>) -> Result<S53, InstantiateErrorKind> {
            Ok(S53(p))
        }
        pub fn t53(InjectTransient(p): InjectTransient<T52>) -> Result<T53, InstantiateErrorKind> {
            Ok(T53(Box::new(p)))
        }
        pub fn s54(Inject(p): Inject<S53>) -> Result<S54, InstantiateErrorKind> {
            Ok(S54(p))
        }
        pub fn t54(InjectTransient(p): InjectTransient<T53>) -> Result<T54, InstantiateErrorKind> {
            Ok(T54(Box::new(p)))
        }
        pub fn s55(Inject(p): Inject<S54>) -> Result<S55, InstantiateErrorKind> {
            Ok(S55(p))
        }
        pub fn t55(InjectTransient(p): InjectTransient<T54>) -> Result<T55, InstantiateErrorKind> {
            Ok(T55(Box::new(p)))
        }
        pub fn s56(Inject(p): Inject<S55>) -> Result<S56, InstantiateErrorKind> {
            Ok(S56(p))
        }
        pub fn t56(InjectTransient(p): InjectTransient<T55>) -> Result<T56, InstantiateErrorKind> {
            Ok(T56(Box::new(p)))
        }
        pub fn s57(Inject(p): Inject<S56>) -> Result<S57, InstantiateErrorKind> {
            Ok(S57(p))
        }
        pub fn t57(InjectTransient(p): InjectTransient<T56>) -> Result<T57, InstantiateErrorKind> {
            Ok(T57(Box::new(p)))
        }
        pub fn s58(Inject(p): Inject<S57>) -> Result<S58, InstantiateErrorKind> {
            Ok(S58(p))
        }
        pub fn t58(InjectTransient(p): InjectTransient<T57>) -> Result<T58, InstantiateErrorKind> {
            Ok(T58(Box::new(p)))
        }
        pub fn s59(Inject(p): Inject<S58>) -> Result<S59, InstantiateErrorKind> {
            Ok(S59(p))
        }
        pub fn t59(InjectTransient(p): InjectTransient<T58>) -> Result<T59, InstantiateErrorKind> {
            Ok(T59(Box::new(p)))
        }
        pub fn s60(Inject(p): Inject<S59>) -> Result<S60, InstantiateErrorKind> {
            Ok(S60(p))
        }
        pub fn t60(InjectTransient(p): InjectTransient<T59>) -> Result<T60, InstantiateErrorKind> {
            Ok(T60(Box::new(p)))
        }
        pub fn s61(Inject(p): Inject<S60>) -> Result<S61, InstantiateErrorKind> {
            Ok(S61(p))
        }
        pub fn t61(InjectTransient(p): InjectTransient<T60>) -> Result<T61, InstantiateErrorKind> {
            Ok(T61(Box::new(p)))
        }
        pub fn s62(Inject(p): Inject<S61>) -> Result<S62, InstantiateErrorKind> {
            Ok(S62(p))
        }
        pub fn t62(InjectTransient(p): InjectTransient<T61>) -> Result<T62, InstantiateErrorKind> {
            Ok(T62(Box::new(p)))
        }
        pub fn s63(Inject(p): Inject<S62>) -> Result<S63, InstantiateErrorKind> {
            Ok(S63(p))
        }
        pub fn t63(InjectTransient(p): InjectTransient<T62>) -> Result<T63, InstantiateErrorKind> {
            Ok(T63(Box::new(p)))
        }
        pub fn s64(Inject(p): Inject<S63>) -> Result<S64, InstantiateErrorKind> {
            Ok(S64(p))
        }
        pub fn t64(InjectTransient(p): InjectTransient<T63>) -> Result<T64, InstantiateErrorKind> {
            Ok(T64(Box::new(p)))
        }
        pub fn s65(Inject(p): Inject<S64>) -> Result<S65, InstantiateErrorKind> {
            Ok(S65(p))
        }
        pub fn t65(InjectTransient(p): InjectTransient<T64>) -> Result<T65, InstantiateErrorKind> {
            Ok(T65(Box::new(p)))
        }
        pub fn s66(Inject(p): Inject<S65>) -> Result<S66, InstantiateErrorKind> {
            Ok(S66(p))
        }
        pub fn t66(InjectTransient(p): InjectTransient<T65>) -> Result<T66, InstantiateErrorKind> {
            Ok(T66(Box::new(p)))
        }
        pub fn s67(Inject(p): Inject<S66>) -> Result<S67, InstantiateErrorKind> {
            Ok(S67(p))
        }
        pub fn t67(InjectTransient(p): InjectTransient<T66>) -> Result<T67, InstantiateErrorKind> {
            Ok(T67(Box::new(p)))
        }
        pub fn s68(Inject(p): Inject<S67>) -> Result<S68, InstantiateErrorKind> {
            Ok(S68(p))
        }
        pub fn t68(InjectTransient(p): InjectTransient<T67>) -> Result<T68, InstantiateErrorKind> {
            Ok(T68(Box::new(p)))
        }
        pub fn s69(Inject(p): Inject<S68>) -> Result<S69, InstantiateErrorKind> {
            Ok(S69(p))
        }
        pub fn t69(InjectTransient(p): InjectTransient<T68>) -> Result<T69, InstantiateErrorKind> {
            Ok(T69(Box::new(p)))
        }
        pub fn s70(Inject(p): Inject<S69>) -> Result<S70, InstantiateErrorKind> {
            Ok(S70(p))
        }
        pub fn t70(InjectTransient(p): InjectTransient<T69>) -> Result<T70, InstantiateErrorKind> {
            Ok(T70(Box::new(p)))
        }
        pub fn s71(Inject(p): Inject<S70>) -> Result<S71, InstantiateErrorKind> {
            Ok(S71(p))
        }
        pub fn t71(InjectTransient(p): InjectTransient<T70>) -> Result<T71, InstantiateErrorKind> {
            Ok(T71(Box::new(p)))
        }
        pub fn s72(Inject(p): Inject<S71>) -> Result<S72, InstantiateErrorKind> {
            Ok(S72(p))
        }
        pub fn t72(InjectTransient(p): InjectTransient<T71>) -> Result<T72, InstantiateErrorKind> {
            Ok(T72(Box::new(p)))
        }
        pub fn s73(Inject(p): Inject<S72>) -> Result<S73, InstantiateErrorKind> {
            Ok(S73(p))
        }
        pub fn t73(InjectTransient(p): InjectTransient<T72>) -> Result<T73, InstantiateErrorKind> {
            Ok(T73(Box::new(p)))
        }
        pub fn s74(Inject(p): Inject<S73>) -> Result<S74, InstantiateErrorKind> {
            Ok(S74(p))
        }
        pub fn t74(InjectTransient(p): InjectTransient<T73>) -> Result<T74, InstantiateErrorKind> {
            Ok(T74(Box::new(p)))
        }
        pub fn s75(Inject(p): Inject<S74>) -> Result<S75, InstantiateErrorKind> {
            Ok(S75(p))
        }
        pub fn t75(InjectTransient(p): InjectTransient<T74>) -> Result<T75, InstantiateErrorKind> {
            Ok(T75(Box::new(p)))
        }
        pub fn s76(Inject(p): Inject<S75>) -> Result<S76, InstantiateErrorKind> {
            Ok(S76(p))
        }
        pub fn t76(InjectTransient(p): InjectTransient<T75>) -> Result<T76, InstantiateErrorKind> {
            Ok(T76(Box::new(p)))
        }
        pub fn s77(Inject(p): Inject<S76>) -> Result<S77, InstantiateErrorKind> {
            Ok(S77(p))
        }
        pub fn t77(InjectTransient(p): InjectTransient<T76>) -> Result<T77, InstantiateErrorKind> {
            Ok(T77(Box::new(p)))
        }
        pub fn s78(Inject(p): Inject<S77>) -> Result<S78, InstantiateErrorKind> {
            Ok(S78(p))
        }
        pub fn t78(InjectTransient(p): InjectTransient<T77>) -> Result<T78, InstantiateErrorKind> {
            Ok(T78(Box::new(p)))
        }
        pub fn s79(Inject(p): Inject<S78>) -> Result<S79, InstantiateErrorKind> {
            Ok(S79(p))
        }
        pub fn t79(InjectTransient(p): InjectTransient<T78>) -> Result<T79, InstantiateErrorKind> {
            Ok(T79(Box::new(p)))
        }
        pub fn s80(Inject(p): Inject<S79>) -> Result<S80, InstantiateErrorKind> {
            Ok(S80(p))
        }
        pub fn t80(InjectTransient(p): InjectTransient<T79>) -> Result<T80, InstantiateErrorKind> {
            Ok(T80(Box::new(p)))
        }
        pub fn s81(Inject(p): Inject<S80>) -> Result<S81, InstantiateErrorKind> {
            Ok(S81(p))
        }
        pub fn t81(InjectTransient(p): InjectTransient<T80>) -> Result<T81, InstantiateErrorKind> {
            Ok(T81(Box::new(p)))
        }
        pub fn s82(Inject(p): Inject<S81>) -> Result<S82, InstantiateErrorKind> {
            Ok(S82(p))
        }
        pub fn t82(InjectTransient(p): InjectTransient<T81>) -> Result<T82, InstantiateErrorKind> {
            Ok(T82(Box::new(p)))
        }
        pub fn s83(Inject(p): Inject<S82>) -> Result<S83, InstantiateErrorKind> {
            Ok(S83(p))
        }
        pub fn t83(InjectTransient(p): InjectTransient<T82>) -> Result<T83, InstantiateErrorKind> {
            Ok(T83(Box::new(p)))
        }
        pub fn s84(Inject(p): Inject<S83>) -> Result<S84, InstantiateErrorKind> {
            Ok(S84(p))
        }
        pub fn t84(InjectTransient(p): InjectTransient<T83>) -> Result<T84, InstantiateErrorKind> {
            Ok(T84(Box::new(p)))
        }
        pub fn s85(Inject(p): Inject<S84>) -> Result<S85, InstantiateErrorKind> {
            Ok(S85(p))
        }
        pub fn t85(InjectTransient(p): InjectTransient<T84>) -> Result<T85, InstantiateErrorKind> {
            Ok(T85(Box::new(p)))
        }
        pub fn s86(Inject(p): Inject<S85>) -> Result<S86, InstantiateErrorKind> {
            Ok(S86(p))
        }
        pub fn t86(InjectTransient(p): InjectTransient<T85>) -> Result<T86, InstantiateErrorKind> {
            Ok(T86(Box::new(p)))
        }
        pub fn s87(Inject(p): Inject<S86>) -> Result<S87, InstantiateErrorKind> {
            Ok(S87(p))
        }
        pub fn t87(InjectTransient(p): InjectTransient<T86>) -> Result<T87, InstantiateErrorKind> {
            Ok(T87(Box::new(p)))
        }
        pub fn s88(Inject(p): Inject<S87>) -> Result<S88, InstantiateErrorKind> {
            Ok(S88(p))
        }
        pub fn t88(InjectTransient(p): InjectTransient<T87>) -> Result<T88, InstantiateErrorKind> {
            Ok(T88(Box::new(p)))
        }
        pub fn s89(Inject(p): Inject<S88>) -> Result<S89, InstantiateErrorKind> {
            Ok(S89(p))
        }
        pub fn t89(InjectTransient(p): InjectTransient<T88>) -> Result<T89, InstantiateErrorKind> {
            Ok(T89(Box::new(p)))
        }
        pub fn s90(Inject(p): Inject<S89>) -> Result<S90, InstantiateErrorKind> {
            Ok(S90(p))
        }
        pub fn t90(InjectTransient(p): InjectTransient<T89>) -> Result<T90, InstantiateErrorKind> {
            Ok(T90(Box::new(p)))
        }
        pub fn s91(Inject(p): Inject<S90>) -> Result<S91, InstantiateErrorKind> {
            Ok(S91(p))
        }
        pub fn t91(InjectTransient(p): InjectTransient<T90>) -> Result<T91, InstantiateErrorKind> {
            Ok(T91(Box::new(p)))
        }
        pub fn s92(Inject(p): Inject<S91>) -> Result<S92, InstantiateErrorKind> {
            Ok(S92(p))
        }
        pub fn t92(InjectTransient(p): InjectTransient<T91>) -> Result<T92, InstantiateErrorKind> {
            Ok(T92(Box::new(p)))
        }
        pub fn s93(Inject(p): Inject<S92>) -> Result<S93, InstantiateErrorKind> {
            Ok(S93(p))
        }
        pub fn t93(InjectTransient(p): InjectTransient<T92>) -> Result<T93, InstantiateErrorKind> {
            Ok(T93(Box::new(p)))
        }
        pub fn s94(Inject(p): Inject<S93>) -> Result<S94, InstantiateErrorKind> {
            Ok(S94(p))
        }
        pub fn t94(InjectTransient(p): InjectTransient<T93>) -> Result<T94, InstantiateErrorKind> {
            Ok(T94(Box::new(p)))
        }
        pub fn s95(Inject(p): Inject<S94>) -> Result<S95, InstantiateErrorKind> {
            Ok(S95(p))
        }
        pub fn t95(InjectTransient(p): InjectTransient<T94>) -> Result<T95, InstantiateErrorKind> {
            Ok(T95(Box::new(p)))
        }
        pub fn s96(Inject(p): Inject<S95>) -> Result<S96, InstantiateErrorKind> {
            Ok(S96(p))
        }
        pub fn t96(InjectTransient(p): InjectTransient<T95>) -> Result<T96, InstantiateErrorKind> {
            Ok(T96(Box::new(p)))
        }
        pub fn s97(Inject(p): Inject<S96>) -> Result<S97, InstantiateErrorKind> {
            Ok(S97(p))
        }
        pub fn t97(InjectTransient(p): InjectTransient<T96>) -> Result<T97, InstantiateErrorKind> {
            Ok(T97(Box::new(p)))
        }
        pub fn s98(Inject(p): Inject<S97>) -> Result<S98, InstantiateErrorKind> {
            Ok(S98(p))
        }
        pub fn t98(InjectTransient(p): InjectTransient<T97>) -> Result<T98, InstantiateErrorKind> {
            Ok(T98(Box::new(p)))
        }
        pub fn s99(Inject(p): Inject<S98>) -> Result<S99, InstantiateErrorKind> {
            Ok(S99(p))
        }
        pub fn t99(InjectTransient(p): InjectTransient<T98>) -> Result<T99, InstantiateErrorKind> {
            Ok(T99(Box::new(p)))
        }
        pub fn wide(
            Inject(_0): Inject<S0>,
            Inject(_1): Inject<S0>,
            Inject(_2): Inject<S0>,
            Inject(_3): Inject<S0>,
            Inject(_4): Inject<S0>,
            Inject(_5): Inject<S0>,
            Inject(_6): Inject<S0>,
            Inject(_7): Inject<S0>,
            Inject(_8): Inject<S0>,
            Inject(_9): Inject<S0>,
            Inject(_10): Inject<S0>,
            Inject(_11): Inject<S0>,
            Inject(_12): Inject<S0>,
            Inject(_13): Inject<S0>,
            Inject(_14): Inject<S0>,
            Inject(_15): Inject<S0>,
        ) -> Result<Wide, InstantiateErrorKind> {
            Ok(Wide([0; 16]))
        }
        pub fn host(Inject(p): Inject<Plugin>) -> Result<Host, InstantiateErrorKind> {
            Ok(Host(p))
        }
    }

    pub fn chain(app: bool) -> Container {
        if app {
            Container::new(
                registry! { scope(App) [ provide(f::s0), provide(f::s1), provide(f::s2), provide(f::s3), provide(f::s4), provide(f::s5), provide(f::s6), provide(f::s7), provide(f::s8), provide(f::s9), provide(f::s10), provide(f::s11), provide(f::s12), provide(f::s13), provide(f::s14), provide(f::s15), provide(f::s16), provide(f::s17), provide(f::s18), provide(f::s19), provide(f::s20), provide(f::s21), provide(f::s22), provide(f::s23), provide(f::s24), provide(f::s25), provide(f::s26), provide(f::s27), provide(f::s28), provide(f::s29), provide(f::s30), provide(f::s31), provide(f::s32), provide(f::s33), provide(f::s34), provide(f::s35), provide(f::s36), provide(f::s37), provide(f::s38), provide(f::s39), provide(f::s40), provide(f::s41), provide(f::s42), provide(f::s43), provide(f::s44), provide(f::s45), provide(f::s46), provide(f::s47), provide(f::s48), provide(f::s49), provide(f::s50), provide(f::s51), provide(f::s52), provide(f::s53), provide(f::s54), provide(f::s55), provide(f::s56), provide(f::s57), provide(f::s58), provide(f::s59), provide(f::s60), provide(f::s61), provide(f::s62), provide(f::s63), provide(f::s64), provide(f::s65), provide(f::s66), provide(f::s67), provide(f::s68), provide(f::s69), provide(f::s70), provide(f::s71), provide(f::s72), provide(f::s73), provide(f::s74), provide(f::s75), provide(f::s76), provide(f::s77), provide(f::s78), provide(f::s79), provide(f::s80), provide(f::s81), provide(f::s82), provide(f::s83), provide(f::s84), provide(f::s85), provide(f::s86), provide(f::s87), provide(f::s88), provide(f::s89), provide(f::s90), provide(f::s91), provide(f::s92), provide(f::s93), provide(f::s94), provide(f::s95), provide(f::s96), provide(f::s97), provide(f::s98), provide(f::s99) ] },
            )
        } else {
            Container::new(
                registry! { scope(Request) [ provide(f::s0), provide(f::s1), provide(f::s2), provide(f::s3), provide(f::s4), provide(f::s5), provide(f::s6), provide(f::s7), provide(f::s8), provide(f::s9), provide(f::s10), provide(f::s11), provide(f::s12), provide(f::s13), provide(f::s14), provide(f::s15), provide(f::s16), provide(f::s17), provide(f::s18), provide(f::s19), provide(f::s20), provide(f::s21), provide(f::s22), provide(f::s23), provide(f::s24), provide(f::s25), provide(f::s26), provide(f::s27), provide(f::s28), provide(f::s29), provide(f::s30), provide(f::s31), provide(f::s32), provide(f::s33), provide(f::s34), provide(f::s35), provide(f::s36), provide(f::s37), provide(f::s38), provide(f::s39), provide(f::s40), provide(f::s41), provide(f::s42), provide(f::s43), provide(f::s44), provide(f::s45), provide(f::s46), provide(f::s47), provide(f::s48), provide(f::s49), provide(f::s50), provide(f::s51), provide(f::s52), provide(f::s53), provide(f::s54), provide(f::s55), provide(f::s56), provide(f::s57), provide(f::s58), provide(f::s59), provide(f::s60), provide(f::s61), provide(f::s62), provide(f::s63), provide(f::s64), provide(f::s65), provide(f::s66), provide(f::s67), provide(f::s68), provide(f::s69), provide(f::s70), provide(f::s71), provide(f::s72), provide(f::s73), provide(f::s74), provide(f::s75), provide(f::s76), provide(f::s77), provide(f::s78), provide(f::s79), provide(f::s80), provide(f::s81), provide(f::s82), provide(f::s83), provide(f::s84), provide(f::s85), provide(f::s86), provide(f::s87), provide(f::s88), provide(f::s89), provide(f::s90), provide(f::s91), provide(f::s92), provide(f::s93), provide(f::s94), provide(f::s95), provide(f::s96), provide(f::s97), provide(f::s98), provide(f::s99) ] },
            )
        }
    }

    pub fn transient_chain() -> Container {
        Container::new(
            registry! { scope(App) [ provide(f::t0), provide(f::t1), provide(f::t2), provide(f::t3), provide(f::t4), provide(f::t5), provide(f::t6), provide(f::t7), provide(f::t8), provide(f::t9), provide(f::t10), provide(f::t11), provide(f::t12), provide(f::t13), provide(f::t14), provide(f::t15), provide(f::t16), provide(f::t17), provide(f::t18), provide(f::t19), provide(f::t20), provide(f::t21), provide(f::t22), provide(f::t23), provide(f::t24), provide(f::t25), provide(f::t26), provide(f::t27), provide(f::t28), provide(f::t29), provide(f::t30), provide(f::t31), provide(f::t32), provide(f::t33), provide(f::t34), provide(f::t35), provide(f::t36), provide(f::t37), provide(f::t38), provide(f::t39), provide(f::t40), provide(f::t41), provide(f::t42), provide(f::t43), provide(f::t44), provide(f::t45), provide(f::t46), provide(f::t47), provide(f::t48), provide(f::t49), provide(f::t50), provide(f::t51), provide(f::t52), provide(f::t53), provide(f::t54), provide(f::t55), provide(f::t56), provide(f::t57), provide(f::t58), provide(f::t59), provide(f::t60), provide(f::t61), provide(f::t62), provide(f::t63), provide(f::t64), provide(f::t65), provide(f::t66), provide(f::t67), provide(f::t68), provide(f::t69), provide(f::t70), provide(f::t71), provide(f::t72), provide(f::t73), provide(f::t74), provide(f::t75), provide(f::t76), provide(f::t77), provide(f::t78), provide(f::t79), provide(f::t80), provide(f::t81), provide(f::t82), provide(f::t83), provide(f::t84), provide(f::t85), provide(f::t86), provide(f::t87), provide(f::t88), provide(f::t89), provide(f::t90), provide(f::t91), provide(f::t92), provide(f::t93), provide(f::t94), provide(f::t95), provide(f::t96), provide(f::t97), provide(f::t98), provide(f::t99) ] },
        )
    }

    pub fn wide() -> Container {
        Container::new(registry! { scope(App) [ provide(f::s0), provide(f::wide) ] })
    }

    pub fn captured() -> Container {
        let label = Arc::new(String::from("captured"));
        Container::new(registry! { scope(Request) [ provide(move || Ok::<_, InstantiateErrorKind>(label.clone())) ] })
    }

    /// The same graphs erased with `into_runtime()`: runtime linking and checked casts.
    pub mod indexed {
        use super::*;

        pub fn chain(app: bool) -> Container {
            let registry = if app {
                registry! { scope(App) [ provide(f::s0), provide(f::s1), provide(f::s2), provide(f::s3), provide(f::s4), provide(f::s5), provide(f::s6), provide(f::s7), provide(f::s8), provide(f::s9), provide(f::s10), provide(f::s11), provide(f::s12), provide(f::s13), provide(f::s14), provide(f::s15), provide(f::s16), provide(f::s17), provide(f::s18), provide(f::s19), provide(f::s20), provide(f::s21), provide(f::s22), provide(f::s23), provide(f::s24), provide(f::s25), provide(f::s26), provide(f::s27), provide(f::s28), provide(f::s29), provide(f::s30), provide(f::s31), provide(f::s32), provide(f::s33), provide(f::s34), provide(f::s35), provide(f::s36), provide(f::s37), provide(f::s38), provide(f::s39), provide(f::s40), provide(f::s41), provide(f::s42), provide(f::s43), provide(f::s44), provide(f::s45), provide(f::s46), provide(f::s47), provide(f::s48), provide(f::s49), provide(f::s50), provide(f::s51), provide(f::s52), provide(f::s53), provide(f::s54), provide(f::s55), provide(f::s56), provide(f::s57), provide(f::s58), provide(f::s59), provide(f::s60), provide(f::s61), provide(f::s62), provide(f::s63), provide(f::s64), provide(f::s65), provide(f::s66), provide(f::s67), provide(f::s68), provide(f::s69), provide(f::s70), provide(f::s71), provide(f::s72), provide(f::s73), provide(f::s74), provide(f::s75), provide(f::s76), provide(f::s77), provide(f::s78), provide(f::s79), provide(f::s80), provide(f::s81), provide(f::s82), provide(f::s83), provide(f::s84), provide(f::s85), provide(f::s86), provide(f::s87), provide(f::s88), provide(f::s89), provide(f::s90), provide(f::s91), provide(f::s92), provide(f::s93), provide(f::s94), provide(f::s95), provide(f::s96), provide(f::s97), provide(f::s98), provide(f::s99) ] }.into_runtime()
            } else {
                registry! { scope(Request) [ provide(f::s0), provide(f::s1), provide(f::s2), provide(f::s3), provide(f::s4), provide(f::s5), provide(f::s6), provide(f::s7), provide(f::s8), provide(f::s9), provide(f::s10), provide(f::s11), provide(f::s12), provide(f::s13), provide(f::s14), provide(f::s15), provide(f::s16), provide(f::s17), provide(f::s18), provide(f::s19), provide(f::s20), provide(f::s21), provide(f::s22), provide(f::s23), provide(f::s24), provide(f::s25), provide(f::s26), provide(f::s27), provide(f::s28), provide(f::s29), provide(f::s30), provide(f::s31), provide(f::s32), provide(f::s33), provide(f::s34), provide(f::s35), provide(f::s36), provide(f::s37), provide(f::s38), provide(f::s39), provide(f::s40), provide(f::s41), provide(f::s42), provide(f::s43), provide(f::s44), provide(f::s45), provide(f::s46), provide(f::s47), provide(f::s48), provide(f::s49), provide(f::s50), provide(f::s51), provide(f::s52), provide(f::s53), provide(f::s54), provide(f::s55), provide(f::s56), provide(f::s57), provide(f::s58), provide(f::s59), provide(f::s60), provide(f::s61), provide(f::s62), provide(f::s63), provide(f::s64), provide(f::s65), provide(f::s66), provide(f::s67), provide(f::s68), provide(f::s69), provide(f::s70), provide(f::s71), provide(f::s72), provide(f::s73), provide(f::s74), provide(f::s75), provide(f::s76), provide(f::s77), provide(f::s78), provide(f::s79), provide(f::s80), provide(f::s81), provide(f::s82), provide(f::s83), provide(f::s84), provide(f::s85), provide(f::s86), provide(f::s87), provide(f::s88), provide(f::s89), provide(f::s90), provide(f::s91), provide(f::s92), provide(f::s93), provide(f::s94), provide(f::s95), provide(f::s96), provide(f::s97), provide(f::s98), provide(f::s99) ] }.into_runtime()
            };
            Container::new(registry! { extend(registry) })
        }

        pub fn transient_chain() -> Container {
            Container::new(
                registry! { extend(registry! { scope(App) [ provide(f::t0), provide(f::t1), provide(f::t2), provide(f::t3), provide(f::t4), provide(f::t5), provide(f::t6), provide(f::t7), provide(f::t8), provide(f::t9), provide(f::t10), provide(f::t11), provide(f::t12), provide(f::t13), provide(f::t14), provide(f::t15), provide(f::t16), provide(f::t17), provide(f::t18), provide(f::t19), provide(f::t20), provide(f::t21), provide(f::t22), provide(f::t23), provide(f::t24), provide(f::t25), provide(f::t26), provide(f::t27), provide(f::t28), provide(f::t29), provide(f::t30), provide(f::t31), provide(f::t32), provide(f::t33), provide(f::t34), provide(f::t35), provide(f::t36), provide(f::t37), provide(f::t38), provide(f::t39), provide(f::t40), provide(f::t41), provide(f::t42), provide(f::t43), provide(f::t44), provide(f::t45), provide(f::t46), provide(f::t47), provide(f::t48), provide(f::t49), provide(f::t50), provide(f::t51), provide(f::t52), provide(f::t53), provide(f::t54), provide(f::t55), provide(f::t56), provide(f::t57), provide(f::t58), provide(f::t59), provide(f::t60), provide(f::t61), provide(f::t62), provide(f::t63), provide(f::t64), provide(f::t65), provide(f::t66), provide(f::t67), provide(f::t68), provide(f::t69), provide(f::t70), provide(f::t71), provide(f::t72), provide(f::t73), provide(f::t74), provide(f::t75), provide(f::t76), provide(f::t77), provide(f::t78), provide(f::t79), provide(f::t80), provide(f::t81), provide(f::t82), provide(f::t83), provide(f::t84), provide(f::t85), provide(f::t86), provide(f::t87), provide(f::t88), provide(f::t89), provide(f::t90), provide(f::t91), provide(f::t92), provide(f::t93), provide(f::t94), provide(f::t95), provide(f::t96), provide(f::t97), provide(f::t98), provide(f::t99) ] }.into_runtime()) },
            )
        }

        pub fn wide() -> Container {
            Container::new(registry! { extend(registry! { scope(App) [ provide(f::s0), provide(f::wide) ] }.into_runtime()) })
        }

        pub fn captured() -> Container {
            let label = Arc::new(String::from("captured"));
            Container::new(
                registry! { extend(registry! { scope(Request) [ provide(move || Ok::<_, InstantiateErrorKind>(label.clone())) ] }.into_runtime()) },
            )
        }
    }

    /// One runtime registration inside a static graph, reached through a declared boundary.
    pub fn mixed() -> Container {
        let plugins = registry! { scope(App) [ provide(|| Ok::<_, InstantiateErrorKind>(Plugin(1))) ] }.into_runtime();
        Container::new(registry! {
            scope(App) [ provide(froodi_compile::runtime::<Plugin>()), provide(f::host) ],
            extend(plugins),
        })
    }
}
