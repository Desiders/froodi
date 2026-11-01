# Froodi-auto

[![Crates.io][crates-badge]][crates-url]

Froodi-auto collects constructors marked with `#[injectable]` into an erased
[Froodi][froodi-crates-url] registry. Auto-registration is explicit: call
`provide_auto_registries()` when building the registry.

```rust
use froodi::{registry, Container, DefaultScope::App, InstantiateErrorKind};
use froodi_auto::{injectable, AutoRegistries as _};

struct Settings;

#[injectable]
impl Settings {
    #[provide(App)]
    fn new() -> Result<Self, InstantiateErrorKind> {
        Ok(Self)
    }
}

fn main() {
    let registry = registry! {}.into_registry().provide_auto_registries();
    let container = Container::new(registry);
    let _settings = container.get::<Settings>().unwrap();
    container.close();
}
```

The `.into_registry()` call is the explicit erasure boundary. These collected
registrations use runtime validation; manual `registry!` registrations retain
their typed checks until erased. With the `async` feature, import
`AutoRegistriesWithSync` and call `.provide_auto_registries_with_sync()` to
collect both sync and async constructors.

## Community
### Telegram
- 🇺🇸 🇷🇺 [@froodi_di](https://t.me/froodi_di)

# Contributing

Contributions are welcome!

## License
[Apache License, Version 2.0][license_apache]

[license_apache]: https://github.com/Desiders/froodi/blob/master/froodi-auto/LICENSE
[crates-badge]: https://img.shields.io/crates/v/froodi-auto.svg
[crates-url]: https://crates.io/crates/froodi-auto
[froodi-crates-url]: https://crates.io/crates/froodi
