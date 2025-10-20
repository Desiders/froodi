use core::{any::TypeId, iter};
use frunk::{HCons, HNil};

use crate::registry_macros::InstantiatorData;

pub(crate) trait Find<Data, Pattern> {
    fn get(&self, pattern: Pattern) -> Option<&Data>;
}

impl<Data, Pattern> Find<Data, Pattern> for HNil {
    fn get(&self, _pattern: Pattern) -> Option<&Data> {
        None
    }
}

impl<Data, Pattern, Head, Tail> Find<Data, Pattern> for HCons<Head, Tail>
where
    Head: Find<Data, Pattern>,
    Tail: Find<Data, Pattern>,
    Pattern: Copy,
{
    fn get(&self, pattern: Pattern) -> Option<&Data> {
        self.head.get(pattern).or(self.tail.get(pattern))
    }
}

impl<Tail> Find<InstantiatorData, TypeId> for HCons<InstantiatorData, Tail>
where
    Tail: Find<InstantiatorData, TypeId>,
{
    fn get(&self, type_id: TypeId) -> Option<&InstantiatorData> {
        if self.head.type_id == type_id {
            Some(&self.head)
        } else {
            self.tail.get(type_id)
        }
    }
}

pub trait Iter<'a, T: 'a> {
    fn iter(&'a self) -> impl Iterator<Item = &'a T>;
}

impl<'a> Iter<'a, InstantiatorData> for frunk::HNil {
    fn iter(&'a self) -> impl Iterator<Item = &'a InstantiatorData> {
        iter::empty()
    }
}

impl<'a, Head, Tail> Iter<'a, InstantiatorData> for frunk::HCons<Head, Tail>
where
    Head: Iter<'a, InstantiatorData>,
    Tail: Iter<'a, InstantiatorData>,
{
    fn iter(&'a self) -> impl Iterator<Item = &'a InstantiatorData> {
        self.head.iter().chain(self.tail.iter())
    }
}

impl<'a, Tail> Iter<'a, InstantiatorData> for frunk::HCons<InstantiatorData, Tail>
where
    Tail: Iter<'a, InstantiatorData>,
{
    fn iter(&'a self) -> impl Iterator<Item = &'a InstantiatorData> {
        iter::once(&self.head).chain(self.tail.iter())
    }
}
