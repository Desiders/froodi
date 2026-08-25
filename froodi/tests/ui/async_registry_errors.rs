//! Every error branch of `async_registry!` / `async_registry_internal!`. Compiled by `trybuild`
//! only with the `async` feature; not a normal test target.
#![allow(unused)]

use froodi::{async_registry, async_registry_internal, Config, DefaultScope::*, InstantiateErrorKind};

async fn inst() -> Result<(), InstantiateErrorKind> {
    Ok(())
}

fn main() {}

// 1. `scope` block must have a scope
fn e01() {
    async_registry! { scope() [ provide(inst) ] };
}
// 2. `scope` block must contain at least one entry
fn e02() {
    async_registry! { scope(App) [] };
}
// 3. Missing comma after `scope` block
fn e03() {
    async_registry! { scope(App) [ provide(inst) ] provide(App, inst) };
}
// 4. Wrong delimiter — parentheses
fn e04() {
    async_registry! { scope(App) ( provide(inst) ) };
}
// 5. Wrong delimiter — braces
fn e05() {
    async_registry! { scope(App) { provide(inst) } };
}
// 6. Multiple scopes in `scope(...)`
fn e06() {
    async_registry! { scope(App, Session) [ provide(inst) ] };
}
// 7. `provide` with no scope or instantiator
fn e07() {
    async_registry! { provide() };
}
// 8. `provide` with a scope but no instantiator
fn e08() {
    async_registry! { provide(App) };
}
// 9. `provide` with no scope before the instantiator
fn e09() {
    async_registry! { provide(, inst) };
}
// 10. Missing comma after a `provide` block
fn e10() {
    async_registry! { provide(App, inst) provide(Session, inst) };
}
// 11. `extend` not in the last position
fn e11() {
    async_registry! { extend(async_registry!()), provide(App, inst) };
}
// 12. `extend` with no arguments
fn e12() {
    async_registry! { extend() };
}
// 13. Missing comma after an `extend` block
fn e13() {
    async_registry! { extend(async_registry!()) provide(App, inst) };
}
// 14. Leading / double comma at the top level
fn e14() {
    async_registry! { , provide(App, inst) };
}
// 15. Lone stray comma
fn e15() {
    async_registry! { , };
}
// 16. Unknown top-level syntax
fn e16() {
    async_registry! { totally bogus };
}
// 17. Double comma after the scope in a top-level `provide`
fn e17() {
    async_registry! { provide(App,, inst) };
}
// 18. Malformed entry list (missing comma between entries)
fn e18() {
    async_registry! { scope(App) [ provide(inst) provide(inst) ] };
}
// 19. Double comma after the instantiator in an entry
fn e19() {
    async_registry! { scope(App) [ provide(inst,, config = Config::default()) ] };
}
// 20. Double comma after `config` in an entry
fn e20() {
    async_registry! { scope(App) [ provide(inst, config = Config::default(),, finalizer = ()) ] };
}
// 21. Double comma after `finalizer` in an entry
fn e21() {
    async_registry! { scope(App) [ provide(inst, finalizer = (),, config = Config::default()) ] };
}
// 22. Double comma after both entry arguments (`config`, then `finalizer`)
fn e22() {
    async_registry! { scope(App) [ provide(inst, config = Config::default(), finalizer = (),, x) ] };
}
// 23. Double comma after both entry arguments (`finalizer`, then `config`)
fn e23() {
    async_registry! { scope(App) [ provide(inst, finalizer = (), config = Config::default(),, x) ] };
}
// 24. Unexpected tokens after the instantiator in an entry
fn e24() {
    async_registry! { scope(App) [ provide(inst, garbage) ] };
}
// 25. `extend` not last, in the middle of the list
fn e25() {
    async_registry! { provide(App, inst), extend(async_registry!()), provide(Session, inst) };
}
// 26. `extend` with no arguments, not first
fn e26() {
    async_registry! { provide(App, inst), extend() };
}
// 27. `extend` block followed by a bracket group
fn e27() {
    async_registry! { extend(async_registry!()) [ x ] };
}
// 28. `scope` with no scope, not first
fn e28() {
    async_registry! { provide(App, inst), scope() [ provide(inst) ] };
}
// 29. empty `scope` block, not first
fn e29() {
    async_registry! { provide(App, inst), scope(Session) [] };
}
// 30. multiple scopes in `scope(...)`, not first
fn e30() {
    async_registry! { provide(App, inst), scope(Session, Request) [ provide(inst) ] };
}
// 31. `provide` with no arguments, not first
fn e31() {
    async_registry! { provide(App, inst), provide() };
}
// 32. `provide` with a scope but no instantiator, not first
fn e32() {
    async_registry! { provide(App, inst), provide(Session) };
}
// 33. `provide` with no scope, not first
fn e33() {
    async_registry! { provide(App, inst), provide(, inst) };
}
// 34. `provide` block followed by a bracket group
fn e34() {
    async_registry! { provide(App, inst) [ x ] };
}
// 35. `scope(...)` with no entry list
fn e35() {
    async_registry! { scope(App) };
}
// 36. unknown clause kind
fn e36() {
    async_registry! { frobnicate(x) };
}
// 37. wrong delimiter on a non-leading clause
fn e37() {
    async_registry! { scope(App) [ provide(inst) ], scope(Session) ( provide(inst) ) };
}
// 38. double comma between clauses
fn e38() {
    async_registry! { provide(App, inst),, provide(Session, inst) };
}
// 39. garbage passed straight to the internal macro
fn e39() {
    async_registry_internal! { bogus };
}
