#![feature(default_type_params)]

#![deny(missing_docs)]
#![deny(warnings)]

//! A type-based key value store where one value type is allowed for each key.

extern crate "unsafe-any" as uany;

use std::any::Any;
use std::intrinsics::TypeId;
use std::collections::{hash_map, HashMap};
use std::hash::{Hash, Hasher, Writer};
use std::mem::transmute;

// These traits are faster when we know the type is correct already.
use uany::{UncheckedAnyDowncast, UncheckedAnyMutDowncast, UncheckedBoxAnyDowncast};

use Entry::{Occupied, Vacant};

/// Custom Hasher that does noop on TypeId
///
/// Copied from https://github.com/chris-morgan/anymap/blob/master/src/lib.rs
#[deriving(Default,Copy)]
pub struct TypeIdHasher;

/// State for Custom Hasher
///
/// Copied from https://github.com/chris-morgan/anymap/blob/master/src/lib.rs
#[deriving(Copy)]
pub struct TypeIdState {
    value: u64,
}

impl Writer for TypeIdState {
    #[inline(always)]
    fn write(&mut self, bytes: &[u8]) {
        // This expects to receive one and exactly one 64-bit value
        debug_assert!(bytes.len() == 8);
        unsafe {
            std::ptr::copy_nonoverlapping_memory(&mut self.value,
                                                 transmute(&bytes[0]),
                                                 1)
        }
    }
}

impl Hasher<TypeIdState> for TypeIdHasher {
    fn hash<Sized? T: Hash<TypeIdState>>(&self, value: &T) -> u64 {
        let mut state = TypeIdState {
            value: 0,
        };
        value.hash(&mut state);
        state.value
    }
}

/// A map keyed by types.
///
/// Can contain one value of any type for each key type, as defined
/// by the Assoc trait.
#[deriving(Default)]
pub struct TypeMap {
    data: HashMap<TypeId, Box<Any + 'static>, TypeIdHasher>
}

/// This trait defines the relationship between keys and values in a TypeMap.
///
/// It is implemented for Keys, with a phantom type parameter for values.
///
/// Even though today's multidispatch rules allow it, this trait must not be
/// implemented more than once by a single type. In the future, the phantom
/// type parameter will move to an associated type.
pub trait Assoc<Value: 'static>: 'static {}

impl TypeMap {
    /// Create a new, empty TypeMap.
    pub fn new() -> TypeMap {
        TypeMap {
            data: HashMap::with_hasher(TypeIdHasher),
        }
    }

    /// Insert a value into the map with a specified key type.
    pub fn insert<K: Assoc<V>, V: 'static>(&mut self, val: V) -> Option<V> {
        self.data.insert(TypeId::of::<(K, V)>(), box val as Box<Any>).map(|v| unsafe {
            *v.downcast_unchecked::<V>()
        })
    }

    /// Find a value in the map and get a reference to it.
    #[deprecated = "renamed to `get`"]
    pub fn find<K: Assoc<V>, V: 'static>(&self) -> Option<&V> {
        self.data.get(&TypeId::of::<(K, V)>()).map(|v| unsafe {
            v.downcast_ref_unchecked::<V>()
        })
    }

    /// Find a value in the map and get a mutable reference to it.
    #[deprecated = "renamed to `get_mut`"]
    pub fn find_mut<K: Assoc<V>, V: 'static>(&mut self) -> Option<&mut V> {
        self.data.get_mut(&TypeId::of::<(K, V)>()).map(|v| unsafe {
            v.downcast_mut_unchecked::<V>()
        })
    }

    /// Find a value in the map and get a reference to it.
    pub fn get<K: Assoc<V>, V: 'static>(&self) -> Option<&V> {
        self.data.get(&TypeId::of::<(K, V)>()).map(|v| unsafe {
            v.downcast_ref_unchecked::<V>()
        })
    }

    /// Find a value in the map and get a mutable reference to it.
    pub fn get_mut<K: Assoc<V>, V: 'static>(&mut self) -> Option<&mut V> {
        self.data.get_mut(&TypeId::of::<(K, V)>()).map(|v| unsafe {
            v.downcast_mut_unchecked::<V>()
        })
    }

    /// Check if a key has an associated value stored in the map.
    pub fn contains<K: Assoc<V>, V: 'static>(&self) -> bool {
        self.data.contains_key(&TypeId::of::<(K, V)>())
    }

    /// Remove a value from the map.
    ///
    /// Returns `true` if a value was removed.
    pub fn remove<K: Assoc<V>, V: 'static>(&mut self) -> Option<V> {
        self.data.remove(&TypeId::of::<(K, V)>()).map(|v| unsafe {
            *v.downcast_unchecked::<V>()
        })
    }

    /// Get the given key's corresponding entry in the map for in-place manipulation.
    pub fn entry<'a, K: Assoc<V>, V: 'static>(&'a mut self) -> Entry<'a, K, V> {
        match self.data.entry(TypeId::of::<(K, V)>()) {
            hash_map::Entry::Occupied(e) => Occupied(OccupiedEntry { data: e }),
            hash_map::Entry::Vacant(e) => Vacant(VacantEntry { data: e })
        }
    }

    /// Read the underlying HashMap
    pub unsafe fn data(&self) -> &HashMap<TypeId, Box<Any + 'static>, TypeIdHasher> { &self.data }

    /// Get a mutable reference to the underlying HashMap
    pub unsafe fn data_mut(&mut self) -> &mut HashMap<TypeId, Box<Any + 'static>, TypeIdHasher> { &mut self.data }

    /// Get the number of values stored in the map.
    pub fn len(&self) -> uint {
        self.data.len()
    }

    /// Return true if the map contains no values.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Remove all entries from the map.
    pub fn clear(&mut self) {
        self.data.clear()
    }
}

/// A view onto an entry in a TypeMap.
pub enum Entry<'a, K, V> {
    /// A view onto an occupied entry in a TypeMap.
    Occupied(OccupiedEntry<'a, K, V>),
    /// A view onto an unoccupied entry in a TypeMap.
    Vacant(VacantEntry<'a, K, V>)
}

/// A view onto an occupied entry in a TypeMap.
pub struct OccupiedEntry<'a, K, V> {
    data: hash_map::OccupiedEntry<'a, TypeId, Box<Any + 'static>>
}

/// A view onto an unoccupied entry in a TypeMap.
pub struct VacantEntry<'a, K, V> {
    data: hash_map::VacantEntry<'a, TypeId, Box<Any + 'static>>
}

impl<'a, K, V: 'static> OccupiedEntry<'a, K, V> {
    /// Get a reference to the entry's value.
    pub fn get(&self) -> &V {
        unsafe {
            self.data.get().downcast_ref_unchecked::<V>()
        }
    }

    /// Get a mutable reference to the entry's value.
    pub fn get_mut(&mut self) -> &mut V {
        unsafe {
            self.data.get_mut().downcast_mut_unchecked::<V>()
        }
    }

    /// Transform the entry into a mutable reference with the same lifetime as the map.
    pub fn into_mut(self) -> &'a mut V {
        unsafe {
            self.data.into_mut().downcast_mut_unchecked::<V>()
        }
    }

    /// Set the entry's value and return the previous value.
    pub fn set(&mut self, value: V) -> V {
        unsafe {
            *self.data.set(box value as Box<Any + 'static>).downcast_unchecked::<V>()
        }
    }

    /// Move the entry's value out of the map, consuming the entry.
    pub fn take(self) -> V {
        unsafe {
            *self.data.take().downcast_unchecked::<V>()
        }
    }
}

impl<'a, K, V: 'static> VacantEntry<'a, K, V> {
    /// Set the entry's value and return a mutable reference to it.
    pub fn set(self, value: V) -> &'a mut V {
        unsafe {
            self.data.set(box value as Box<Any + 'static>).downcast_mut_unchecked::<V>()
        }
    }
}

#[cfg(test)]
mod test {
    use super::{TypeMap, Assoc};
    use super::Entry::{Occupied, Vacant};

    #[deriving(Show, PartialEq)]
    struct Key;

    #[deriving(Show, PartialEq)]
    struct Value;

    impl Assoc<Value> for Key {}

    #[test] fn test_pairing() {
        let mut map = TypeMap::new();
        map.insert::<Key, Value>(Value);
        assert_eq!(*map.get::<Key, Value>().unwrap(), Value);
        assert!(map.contains::<Key, Value>());
    }

    #[test] fn test_remove() {
        let mut map = TypeMap::new();
        map.insert::<Key, Value>(Value);
        assert!(map.contains::<Key, Value>());
        map.remove::<Key, Value>();
        assert!(!map.contains::<Key, Value>());
    }

    #[test] fn test_entry() {
        let mut map = TypeMap::new();
        map.insert::<Key, Value>(Value);
        match map.entry::<Key, Value>() {
            Occupied(e) => {
                assert_eq!(e.get(), &Value);
                assert_eq!(e.take(), Value);
            },
            _ => panic!("Unable to locate inserted item.")
        }
        assert!(!map.contains::<Key, Value>());
        match map.entry::<Key, Value>() {
            Vacant(e) => {
                e.set(Value);
            },
            _ => panic!("Found non-existant entry.")
        }
        assert!(map.contains::<Key, Value>());
    }

    #[test] fn test_entry_multi() {
        impl Assoc<f64> for Key {}
        impl Assoc<u32> for Key {}

        let mut map = TypeMap::new();
        map.insert::<Key, u32>(44);
        map.insert::<Key, Value>(Value);
        if let Occupied(_) = map.entry::<Key, f64>() {
            panic!("Unsound")
        }

        assert_eq!(*map.get::<Key, Value>().unwrap(), Value);
        map.remove::<Key, Value>();
        assert!(!map.contains::<Key, Value>());

        assert_eq!(*map.get::<Key, u32>().unwrap(), 44);
        map.remove::<Key, u32>();
        assert!(!map.contains::<Key, u32>());
    }
}

