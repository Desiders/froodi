use alloc::collections::BTreeMap;
use core::{
    any::{type_name, TypeId},
    cmp::Ordering,
    fmt::{self, Display, Formatter},
    hash::{Hash, Hasher},
};

use crate::utils::thread_safety::RcAnyThreadSafety;

#[derive(Debug, Clone)]
pub struct TypeInfo {
    pub name: &'static str,
    pub id: TypeId,
}

impl PartialEq for TypeInfo {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for TypeInfo {}

impl PartialOrd for TypeInfo {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TypeInfo {
    fn cmp(&self, other: &Self) -> Ordering {
        self.id.cmp(&other.id)
    }
}

impl Hash for TypeInfo {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl Display for TypeInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl TypeInfo {
    #[inline]
    #[must_use]
    #[cfg(const_type_id)]
    pub(crate) const fn new<T: ?Sized + 'static>(name: &'static str) -> Self {
        Self {
            name,
            id: TypeId::of::<T>(),
        }
    }

    #[inline]
    #[must_use]
    #[cfg(not(const_type_id))]
    pub(crate) fn new<T: ?Sized + 'static>(name: &'static str) -> Self {
        Self {
            name,
            id: TypeId::of::<T>(),
        }
    }

    #[inline]
    #[must_use]
    pub(crate) fn of<T>() -> Self
    where
        T: ?Sized + 'static,
    {
        Self {
            name: type_name::<T>(),
            id: TypeId::of::<T>(),
        }
    }

    #[inline]
    #[must_use]
    pub(crate) fn of_val<T>(_val: &T) -> Self
    where
        T: ?Sized + 'static,
    {
        Self {
            name: type_name::<T>(),
            id: TypeId::of::<T>(),
        }
    }

    #[inline]
    #[must_use]
    pub(crate) fn short_name(&self) -> &'static str {
        self.name.rsplit_once("::").map_or(self.name, |(_, name)| name)
    }
}

pub(crate) type Map = BTreeMap<TypeInfo, RcAnyThreadSafety>;

#[cfg(test)]
mod tests {
    extern crate std;

    use super::TypeInfo;
    use alloc::{collections::BTreeMap, format, string::ToString as _};
    use core::{
        cmp::Ordering,
        hash::{Hash as _, Hasher as _},
    };

    struct Foo;
    struct Bar;
    struct Baz(#[allow(dead_code)] u32);

    #[test]
    fn test_of_same_type_equal() {
        let a = TypeInfo::of::<Foo>();
        let b = TypeInfo::of::<Foo>();

        assert_eq!(a, b);
        assert_eq!(a.cmp(&b), Ordering::Equal);
        assert_eq!(a.partial_cmp(&b), Some(Ordering::Equal));
        assert_eq!(a.id, b.id);
    }

    #[test]
    fn test_of_different_types_not_equal() {
        let a = TypeInfo::of::<Foo>();
        let b = TypeInfo::of::<Bar>();

        assert_ne!(a, b);
        assert_ne!(a.id, b.id);
        assert_ne!(a.cmp(&b), Ordering::Equal);
        // cmp is antisymmetric.
        assert_eq!(a.cmp(&b), b.cmp(&a).reverse());
    }

    #[test]
    fn test_ord_consistent_with_partial_cmp() {
        let a = TypeInfo::of::<Foo>();
        let b = TypeInfo::of::<Bar>();
        let c = TypeInfo::of::<Baz>();

        for (x, y) in [(&a, &b), (&b, &c), (&a, &c), (&a, &a)] {
            assert_eq!(x.partial_cmp(y), Some(x.cmp(y)));
        }
    }

    #[test]
    fn test_btreemap_key_distinct_types() {
        let mut map = BTreeMap::new();
        map.insert(TypeInfo::of::<Foo>(), 1_u32);
        map.insert(TypeInfo::of::<Bar>(), 2_u32);

        assert_eq!(map.len(), 2);
        assert_eq!(map.get(&TypeInfo::of::<Foo>()), Some(&1));
        assert_eq!(map.get(&TypeInfo::of::<Bar>()), Some(&2));
        assert_eq!(map.get(&TypeInfo::of::<Baz>()), None);
    }

    #[test]
    fn test_btreemap_same_type_overwrites() {
        let mut map = BTreeMap::new();
        map.insert(TypeInfo::of::<Foo>(), 1_u32);
        let previous = map.insert(TypeInfo::of::<Foo>(), 42_u32);

        assert_eq!(previous, Some(1));
        assert_eq!(map.len(), 1);
        assert_eq!(map.get(&TypeInfo::of::<Foo>()), Some(&42));
    }

    #[test]
    fn test_hash_equal_for_equal_values() {
        // Deterministic no_std hasher to exercise the Hash impl.
        #[derive(Default)]
        struct CountingHasher(u64);
        impl core::hash::Hasher for CountingHasher {
            fn finish(&self) -> u64 {
                self.0
            }
            fn write(&mut self, bytes: &[u8]) {
                for &b in bytes {
                    self.0 = self.0.wrapping_mul(31).wrapping_add(u64::from(b));
                }
            }
        }

        fn hash_of(ti: &TypeInfo) -> u64 {
            let mut h = CountingHasher::default();
            ti.hash(&mut h);
            h.finish()
        }

        let a = TypeInfo::of::<Foo>();
        let b = TypeInfo::of::<Foo>();
        let c = TypeInfo::of::<Bar>();

        assert_eq!(a, b);
        assert_eq!(hash_of(&a), hash_of(&b));
        // Only equality is asserted for distinct types; hash inequality is not guaranteed.
        assert_ne!(a, c);
    }

    #[test]
    fn test_display_contains_type_name() {
        let display = format!("{}", TypeInfo::of::<Foo>());
        assert_eq!(display, TypeInfo::of::<Foo>().name);
        assert!(display.contains("Foo"), "display = {display}");
        let other = TypeInfo::of::<Bar>().to_string();
        assert!(other.contains("Bar"), "other = {other}");
        assert_ne!(display, other);
    }

    #[test]
    fn test_of_val_matches_of() {
        let value = Baz(7);
        let from_val = TypeInfo::of_val(&value);
        let from_type = TypeInfo::of::<Baz>();

        assert_eq!(from_val, from_type);
        assert_eq!(from_val.id, from_type.id);
        assert_eq!(from_val.name, from_type.name);
        assert_ne!(from_val, TypeInfo::of::<Foo>());
    }

    #[test]
    fn test_short_name_is_last_segment() {
        let ti = TypeInfo::of::<Foo>();
        let short = ti.short_name();

        // type_name for a local test struct looks like "...::any::tests::Foo".
        assert!(ti.name.contains("::"), "name = {}", ti.name);
        assert_eq!(short, "Foo");
        assert!(!short.contains("::"), "short = {short}");
        assert!(ti.name.ends_with(short), "name = {}, short = {short}", ti.name);
    }

    #[test]
    fn test_short_name_no_path() {
        // A primitive's type_name has no "::", so short_name returns it unchanged.
        let ti = TypeInfo::of::<u32>();
        assert_eq!(ti.name, "u32");
        assert_eq!(ti.short_name(), "u32");
    }
}
