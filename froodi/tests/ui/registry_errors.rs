//! Every error branch of `registry!` / `registry_internal!`. Compiled by `trybuild`; not a normal
//! test target (cargo does not auto-discover files under `tests/ui/`).
#![allow(unused)]

use froodi::{registry, Config, DefaultScope::*, InstantiateErrorKind};

fn inst() -> Result<(), InstantiateErrorKind> {
    Ok(())
}

fn main() {}

// 1. `scope` block must have a scope
fn e01() {
    registry! { scope() [ provide(inst) ] };
}
// 2. `scope` block must contain at least one entry
fn e02() {
    registry! { scope(App) [] };
}
// 3. Missing comma after `scope` block
fn e03() {
    registry! { scope(App) [ provide(inst) ] provide(App, inst) };
}
// 4. Wrong delimiter — parentheses
fn e04() {
    registry! { scope(App) ( provide(inst) ) };
}
// 5. Wrong delimiter — braces
fn e05() {
    registry! { scope(App) { provide(inst) } };
}
// 6. Multiple scopes in `scope(...)`
fn e06() {
    registry! { scope(App, Session) [ provide(inst) ] };
}
// 7. `provide` with no scope or instantiator
fn e07() {
    registry! { provide() };
}
// 8. `provide` with a scope but no instantiator
fn e08() {
    registry! { provide(App) };
}
// 9. `provide` with no scope before the instantiator
fn e09() {
    registry! { provide(, inst) };
}
// 10. Missing comma after a `provide` block
fn e10() {
    registry! { provide(App, inst) provide(Session, inst) };
}
// 11. `extend` not in the last position
fn e11() {
    registry! { extend(registry!()), provide(App, inst) };
}
// 12. `extend` with no arguments
fn e12() {
    registry! { extend() };
}
// 13. Missing comma after an `extend` block
fn e13() {
    registry! { extend(registry!()) provide(App, inst) };
}
// 14. Leading / double comma at the top level
fn e14() {
    registry! { , provide(App, inst) };
}
// 15. Lone stray comma
fn e15() {
    registry! { , };
}
// 16. Unknown top-level syntax
fn e16() {
    registry! { totally bogus };
}
// 17. Double comma after the scope in a top-level `provide`
fn e17() {
    registry! { provide(App,, inst) };
}
// 18. Malformed entry list (missing comma between entries)
fn e18() {
    registry! { scope(App) [ provide(inst) provide(inst) ] };
}
// 19. Double comma after the instantiator in an entry
fn e19() {
    registry! { scope(App) [ provide(inst,, config = Config::default()) ] };
}
// 20. Double comma after `config` in an entry
fn e20() {
    registry! { scope(App) [ provide(inst, config = Config::default(),, finalizer = ()) ] };
}
// 21. Double comma after `finalizer` in an entry
fn e21() {
    registry! { scope(App) [ provide(inst, finalizer = (),, config = Config::default()) ] };
}
// 22. Double comma after both entry arguments (`config`, then `finalizer`)
fn e22() {
    registry! { scope(App) [ provide(inst, config = Config::default(), finalizer = (),, x) ] };
}
// 23. Double comma after both entry arguments (`finalizer`, then `config`)
fn e23() {
    registry! { scope(App) [ provide(inst, finalizer = (), config = Config::default(),, x) ] };
}
// 24. Unexpected tokens after the instantiator in an entry
fn e24() {
    registry! { scope(App) [ provide(inst, garbage) ] };
}
