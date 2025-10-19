use core::any::TypeId;

use frunk::{HCons, HNil};

use crate::registry_macros::InstantiatorData;

pub(crate) trait HListFind<Data, Pattern> {
    fn get(&self, pattern: Pattern) -> Option<&Data>;
}

impl<Data, Pattern> HListFind<Data, Pattern> for HNil {
    fn get(&self, _pattern: Pattern) -> Option<&Data> {
        None
    }
}

impl<Data, Pattern, Head, Tail> HListFind<Data, Pattern> for HCons<Head, Tail>
where
    Head: HListFind<Data, Pattern>,
    Tail: HListFind<Data, Pattern>,
    Pattern: Copy,
{
    fn get(&self, pattern: Pattern) -> Option<&Data> {
        self.head.get(pattern).or(self.tail.get(pattern))
    }
}

impl<Tail> HListFind<InstantiatorData, TypeId> for HCons<InstantiatorData, Tail>
where
    Tail: HListFind<InstantiatorData, TypeId>,
{
    fn get(&self, type_id: TypeId) -> Option<&InstantiatorData> {
        if self.head.type_id == type_id {
            Some(&self.head)
        } else {
            self.tail.get(type_id)
        }
    }
}
