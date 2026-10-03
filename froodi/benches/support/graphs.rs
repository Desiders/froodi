//! Chains contain 100 distinct values: Arc injection or immediate transient ownership.
#![allow(dead_code)]

use froodi::{Inject, InjectTransient, InstantiateErrorKind};
use std::sync::Arc;

#[derive(Clone)]
pub struct S0;
pub struct T0;

macro_rules! chain {
    ($($cached:ident $fresh:ident $previous:ident $previous_fresh:ident $inst:ident $fresh_inst:ident),+) => {
        $(pub struct $cached(pub Arc<dyn Send + Sync>);
        pub struct $fresh(pub Box<dyn Send + Sync>);)+

        mod inst {
            use super::*;

            pub fn s0() -> Result<S0, InstantiateErrorKind> { Ok(S0) }

            pub fn t0() -> Result<T0, InstantiateErrorKind> { Ok(T0) }

            $(pub fn $inst(Inject(previous): Inject<$previous>) -> Result<$cached, InstantiateErrorKind> {
                Ok($cached(previous))
            }

            pub fn $fresh_inst(InjectTransient(previous): InjectTransient<$previous_fresh>) -> Result<$fresh, InstantiateErrorKind> {
                Ok($fresh(Box::new(previous)))
            })+
        }
    };
}

chain!(
        S1 T1 S0 T0 s1 t1,
        S2 T2 S1 T1 s2 t2,
        S3 T3 S2 T2 s3 t3,
        S4 T4 S3 T3 s4 t4,
        S5 T5 S4 T4 s5 t5,
        S6 T6 S5 T5 s6 t6,
        S7 T7 S6 T6 s7 t7,
        S8 T8 S7 T7 s8 t8,
        S9 T9 S8 T8 s9 t9,
        S10 T10 S9 T9 s10 t10,
        S11 T11 S10 T10 s11 t11,
        S12 T12 S11 T11 s12 t12,
        S13 T13 S12 T12 s13 t13,
        S14 T14 S13 T13 s14 t14,
        S15 T15 S14 T14 s15 t15,
        S16 T16 S15 T15 s16 t16,
        S17 T17 S16 T16 s17 t17,
        S18 T18 S17 T17 s18 t18,
        S19 T19 S18 T18 s19 t19,
        S20 T20 S19 T19 s20 t20,
        S21 T21 S20 T20 s21 t21,
        S22 T22 S21 T21 s22 t22,
        S23 T23 S22 T22 s23 t23,
        S24 T24 S23 T23 s24 t24,
        S25 T25 S24 T24 s25 t25,
        S26 T26 S25 T25 s26 t26,
        S27 T27 S26 T26 s27 t27,
        S28 T28 S27 T27 s28 t28,
        S29 T29 S28 T28 s29 t29,
        S30 T30 S29 T29 s30 t30,
        S31 T31 S30 T30 s31 t31,
        S32 T32 S31 T31 s32 t32,
        S33 T33 S32 T32 s33 t33,
        S34 T34 S33 T33 s34 t34,
        S35 T35 S34 T34 s35 t35,
        S36 T36 S35 T35 s36 t36,
        S37 T37 S36 T36 s37 t37,
        S38 T38 S37 T37 s38 t38,
        S39 T39 S38 T38 s39 t39,
        S40 T40 S39 T39 s40 t40,
        S41 T41 S40 T40 s41 t41,
        S42 T42 S41 T41 s42 t42,
        S43 T43 S42 T42 s43 t43,
        S44 T44 S43 T43 s44 t44,
        S45 T45 S44 T44 s45 t45,
        S46 T46 S45 T45 s46 t46,
        S47 T47 S46 T46 s47 t47,
        S48 T48 S47 T47 s48 t48,
        S49 T49 S48 T48 s49 t49,
        S50 T50 S49 T49 s50 t50,
        S51 T51 S50 T50 s51 t51,
        S52 T52 S51 T51 s52 t52,
        S53 T53 S52 T52 s53 t53,
        S54 T54 S53 T53 s54 t54,
        S55 T55 S54 T54 s55 t55,
        S56 T56 S55 T55 s56 t56,
        S57 T57 S56 T56 s57 t57,
        S58 T58 S57 T57 s58 t58,
        S59 T59 S58 T58 s59 t59,
        S60 T60 S59 T59 s60 t60,
        S61 T61 S60 T60 s61 t61,
        S62 T62 S61 T61 s62 t62,
        S63 T63 S62 T62 s63 t63,
        S64 T64 S63 T63 s64 t64,
        S65 T65 S64 T64 s65 t65,
        S66 T66 S65 T65 s66 t66,
        S67 T67 S66 T66 s67 t67,
        S68 T68 S67 T67 s68 t68,
        S69 T69 S68 T68 s69 t69,
        S70 T70 S69 T69 s70 t70,
        S71 T71 S70 T70 s71 t71,
        S72 T72 S71 T71 s72 t72,
        S73 T73 S72 T72 s73 t73,
        S74 T74 S73 T73 s74 t74,
        S75 T75 S74 T74 s75 t75,
        S76 T76 S75 T75 s76 t76,
        S77 T77 S76 T76 s77 t77,
        S78 T78 S77 T77 s78 t78,
        S79 T79 S78 T78 s79 t79,
        S80 T80 S79 T79 s80 t80,
        S81 T81 S80 T80 s81 t81,
        S82 T82 S81 T81 s82 t82,
        S83 T83 S82 T82 s83 t83,
        S84 T84 S83 T83 s84 t84,
        S85 T85 S84 T84 s85 t85,
        S86 T86 S85 T85 s86 t86,
        S87 T87 S86 T86 s87 t87,
        S88 T88 S87 T87 s88 t88,
        S89 T89 S88 T88 s89 t89,
        S90 T90 S89 T89 s90 t90,
        S91 T91 S90 T90 s91 t91,
        S92 T92 S91 T91 s92 t92,
        S93 T93 S92 T92 s93 t93,
        S94 T94 S93 T93 s94 t94,
        S95 T95 S94 T94 s95 t95,
        S96 T96 S95 T95 s96 t96,
        S97 T97 S96 T96 s97 t97,
        S98 T98 S97 T97 s98 t98,
        S99 T99 S98 T98 s99 t99
);

pub mod typed {
    use super::inst;
    use froodi::{
        registry, Container,
        DefaultScope::{App, Request},
    };

    pub fn chain(app: bool) -> Container {
        let scope = if app { App } else { Request };
        Container::new(
            registry! { scope(scope) [ provide(inst::s0), provide(inst::s1), provide(inst::s2), provide(inst::s3), provide(inst::s4), provide(inst::s5), provide(inst::s6), provide(inst::s7), provide(inst::s8), provide(inst::s9), provide(inst::s10), provide(inst::s11), provide(inst::s12), provide(inst::s13), provide(inst::s14), provide(inst::s15), provide(inst::s16), provide(inst::s17), provide(inst::s18), provide(inst::s19), provide(inst::s20), provide(inst::s21), provide(inst::s22), provide(inst::s23), provide(inst::s24), provide(inst::s25), provide(inst::s26), provide(inst::s27), provide(inst::s28), provide(inst::s29), provide(inst::s30), provide(inst::s31), provide(inst::s32), provide(inst::s33), provide(inst::s34), provide(inst::s35), provide(inst::s36), provide(inst::s37), provide(inst::s38), provide(inst::s39), provide(inst::s40), provide(inst::s41), provide(inst::s42), provide(inst::s43), provide(inst::s44), provide(inst::s45), provide(inst::s46), provide(inst::s47), provide(inst::s48), provide(inst::s49), provide(inst::s50), provide(inst::s51), provide(inst::s52), provide(inst::s53), provide(inst::s54), provide(inst::s55), provide(inst::s56), provide(inst::s57), provide(inst::s58), provide(inst::s59), provide(inst::s60), provide(inst::s61), provide(inst::s62), provide(inst::s63), provide(inst::s64), provide(inst::s65), provide(inst::s66), provide(inst::s67), provide(inst::s68), provide(inst::s69), provide(inst::s70), provide(inst::s71), provide(inst::s72), provide(inst::s73), provide(inst::s74), provide(inst::s75), provide(inst::s76), provide(inst::s77), provide(inst::s78), provide(inst::s79), provide(inst::s80), provide(inst::s81), provide(inst::s82), provide(inst::s83), provide(inst::s84), provide(inst::s85), provide(inst::s86), provide(inst::s87), provide(inst::s88), provide(inst::s89), provide(inst::s90), provide(inst::s91), provide(inst::s92), provide(inst::s93), provide(inst::s94), provide(inst::s95), provide(inst::s96), provide(inst::s97), provide(inst::s98), provide(inst::s99) ] },
        )
    }

    pub fn transient_chain() -> Container {
        Container::new(
            registry! { scope(App) [ provide(inst::t0), provide(inst::t1), provide(inst::t2), provide(inst::t3), provide(inst::t4), provide(inst::t5), provide(inst::t6), provide(inst::t7), provide(inst::t8), provide(inst::t9), provide(inst::t10), provide(inst::t11), provide(inst::t12), provide(inst::t13), provide(inst::t14), provide(inst::t15), provide(inst::t16), provide(inst::t17), provide(inst::t18), provide(inst::t19), provide(inst::t20), provide(inst::t21), provide(inst::t22), provide(inst::t23), provide(inst::t24), provide(inst::t25), provide(inst::t26), provide(inst::t27), provide(inst::t28), provide(inst::t29), provide(inst::t30), provide(inst::t31), provide(inst::t32), provide(inst::t33), provide(inst::t34), provide(inst::t35), provide(inst::t36), provide(inst::t37), provide(inst::t38), provide(inst::t39), provide(inst::t40), provide(inst::t41), provide(inst::t42), provide(inst::t43), provide(inst::t44), provide(inst::t45), provide(inst::t46), provide(inst::t47), provide(inst::t48), provide(inst::t49), provide(inst::t50), provide(inst::t51), provide(inst::t52), provide(inst::t53), provide(inst::t54), provide(inst::t55), provide(inst::t56), provide(inst::t57), provide(inst::t58), provide(inst::t59), provide(inst::t60), provide(inst::t61), provide(inst::t62), provide(inst::t63), provide(inst::t64), provide(inst::t65), provide(inst::t66), provide(inst::t67), provide(inst::t68), provide(inst::t69), provide(inst::t70), provide(inst::t71), provide(inst::t72), provide(inst::t73), provide(inst::t74), provide(inst::t75), provide(inst::t76), provide(inst::t77), provide(inst::t78), provide(inst::t79), provide(inst::t80), provide(inst::t81), provide(inst::t82), provide(inst::t83), provide(inst::t84), provide(inst::t85), provide(inst::t86), provide(inst::t87), provide(inst::t88), provide(inst::t89), provide(inst::t90), provide(inst::t91), provide(inst::t92), provide(inst::t93), provide(inst::t94), provide(inst::t95), provide(inst::t96), provide(inst::t97), provide(inst::t98), provide(inst::t99) ] },
        )
    }
}
