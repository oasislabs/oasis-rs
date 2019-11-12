use std::{
    borrow::Borrow,
    fmt::{self, Debug},
};

use borsh::{BorshDeserialize, BorshSerialize};

/// `Map` is a data structure with a [`HashMap`](https://doc.rust-lang.org/std/collections/hash_map/struct.HashMap.html)-like API but based on a `Vec`.
/// It's primarily useful when you care about constant factors or prefer determinism to speed.
/// Please refer to the [docs for `HashMap`](https://doc.rust-lang.org/std/collections/hash_map/struct.HashMap.html) for details and examples of the Map API.
///
/// ## Example
///
/// ```
/// use oasis_std::collections::Map;
/// let mut map = Map::new();
/// map.insert("hello".to_string(), "world".to_string());
/// map.entry("hello".to_string())
///     .and_modify(|mut v| v.push_str("!"));
/// assert_eq!(map.get("hello").map(String::as_str), Some("world!"))
/// ```
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Map<K, V> {
    backing: Vec<(K, V)>,
}

impl<K: Eq, V> Map<K, V> {
    pub fn new() -> Self {
        Self {
            backing: Vec::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            backing: Vec::with_capacity(capacity),
        }
    }

    pub fn capacity(&self) -> usize {
        self.backing.capacity()
    }

    pub fn clear(&mut self) {
        self.backing.clear()
    }

    pub fn contains_key<Q: ?Sized>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Eq,
    {
        self.keys().any(|k| key.eq(k.borrow()))
    }

    pub fn drain(&mut self) -> std::vec::Drain<(K, V)> {
        self.backing.drain(..)
    }

    pub fn entry(&mut self, key: K) -> Entry<'_, K, V> {
        match self.backing.iter_mut().position(|(k, _)| *k == key) {
            Some(pos) => Entry::Occupied(OccupiedEntry {
                entry_pos: pos,
                // entry: unsafe { core::mem::transmute::<&mut (K, V), &'a mut (K, V)>(entry) },
                /* ^ since the only operations on an OccupiedEntry modify `v` in-place, the Vec will
                 * never move in memory (reallocte), so the ref is valid for the duration of the OE. */
                backing: &mut self.backing,
            }),
            None => Entry::Vacant(VacantEntry {
                key,
                backing: &mut self.backing,
            }),
        }
    }

    pub fn get<Q: ?Sized>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Eq,
    {
        self.backing
            .iter()
            .find(|(k, _)| key.eq(k.borrow()))
            .map(|(_, v)| v)
    }

    pub fn get_key_value<Q: ?Sized>(&self, key: &Q) -> Option<(&K, &V)>
    where
        K: Borrow<Q>,
        Q: Eq,
    {
        self.backing
            .iter()
            .find(|(k, _)| key.eq(k.borrow()))
            .map(|(k, v)| (k, v))
    }

    pub fn get_mut<Q: ?Sized>(&mut self, key: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Eq,
    {
        self.backing
            .iter_mut()
            .find(|(k, _)| key.eq(k.borrow()))
            .map(|(_, v)| v)
    }

    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        for (k, ref mut v) in self.backing.iter_mut() {
            if *k == key {
                return Some(core::mem::replace(v, value));
            }
        }
        self.backing.push((key, value));
        None
    }

    pub fn is_empty(&self) -> bool {
        self.backing.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> + DoubleEndedIterator + ExactSizeIterator {
        self.backing.iter().map(|(k, v)| (k, v))
    }

    pub fn iter_mut(
        &mut self,
    ) -> impl Iterator<Item = (&mut K, &mut V)> + DoubleEndedIterator + ExactSizeIterator {
        self.backing.iter_mut().map(|(k, v)| (k, v))
    }

    pub fn keys(&self) -> impl Iterator<Item = &K> + DoubleEndedIterator + ExactSizeIterator {
        self.backing.iter().map(|(k, _)| k)
    }

    pub fn len(&self) -> usize {
        self.backing.len()
    }

    pub fn remove<Q: ?Sized>(&mut self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Eq,
    {
        self.remove_entry(key).map(|(_, v)| v)
    }

    pub fn remove_entry<Q: ?Sized>(&mut self, key: &Q) -> Option<(K, V)>
    where
        K: Borrow<Q>,
        Q: Eq,
    {
        self.backing
            .iter()
            .position(|(k, _)| key.eq(k.borrow()))
            .map(|pos| self.backing.remove(pos))
    }

    pub fn reserve(&mut self, additional: usize) {
        self.backing.reserve(additional)
    }

    pub fn retain(&mut self, mut f: impl FnMut(&K, &mut V) -> bool) {
        self.backing.drain_filter(|(k, ref mut v)| !f(k, v));
    }

    pub fn shrink_to(&mut self, min_capacity: usize) {
        self.backing.shrink_to(min_capacity)
    }

    pub fn shrink_to_fit(&mut self) {
        self.backing.shrink_to_fit()
    }

    pub fn try_reserve(
        &mut self,
        additional: usize,
    ) -> Result<(), std::collections::TryReserveError> {
        self.backing.try_reserve(additional)
    }

    pub fn values(&self) -> impl Iterator<Item = &V> + DoubleEndedIterator + ExactSizeIterator {
        self.backing.iter().map(|(_, v)| v)
    }

    pub fn values_mut(
        &mut self,
    ) -> impl Iterator<Item = &mut V> + DoubleEndedIterator + ExactSizeIterator {
        self.backing.iter_mut().map(|(_, v)| v)
    }
}

impl<K: Debug, V: Debug> fmt::Debug for Map<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map()
            .entries(self.backing.iter().map(|&(ref k, ref v)| (k, v)))
            .finish()
    }
}

type MapRefIntoTuple<'a, K, V, I> = core::iter::Map<I, fn(&'a (K, V)) -> (&'a K, &'a V)>;
type MapMutRefIntoTuple<'a, K, V, I> =
    core::iter::Map<I, fn(&'a mut (K, V)) -> (&'a mut K, &'a mut V)>;

impl<'a, K, V> IntoIterator for &'a Map<K, V> {
    type Item = (&'a K, &'a V);
    type IntoIter = MapRefIntoTuple<'a, K, V, core::slice::Iter<'a, (K, V)>>;

    fn into_iter(self) -> <Self as IntoIterator>::IntoIter {
        self.backing.iter().map(|(k, v)| (k, v))
    }
}

impl<'a, K, V> IntoIterator for &'a mut Map<K, V> {
    type Item = (&'a mut K, &'a mut V);
    type IntoIter = MapMutRefIntoTuple<'a, K, V, core::slice::IterMut<'a, (K, V)>>;

    fn into_iter(self) -> <Self as IntoIterator>::IntoIter {
        self.backing.iter_mut().map(|(k, v)| (k, v))
    }
}

impl<K, V> IntoIterator for Map<K, V> {
    type Item = (K, V);
    type IntoIter = std::vec::IntoIter<(K, V)>;

    fn into_iter(self) -> <Self as IntoIterator>::IntoIter {
        self.backing.into_iter()
    }
}

impl<K: Eq, V> core::iter::FromIterator<(K, V)> for Map<K, V> {
    fn from_iter<T: IntoIterator<Item = (K, V)>>(iter: T) -> Self {
        let mut this = Self::new();
        this.extend(iter);
        this
    }
}

impl<K: Eq, V> Extend<(K, V)> for Map<K, V> {
    fn extend<T: IntoIterator<Item = (K, V)>>(&mut self, iter: T) {
        for (k, v) in iter {
            self.insert(k, v);
        }
    }
}

impl<'a, K: 'a + Copy + Eq, V: 'a + Copy> Extend<(&'a K, &'a V)> for Map<K, V> {
    fn extend<T: IntoIterator<Item = (&'a K, &'a V)>>(&mut self, iter: T) {
        for (k, v) in iter {
            self.insert(*k, *v);
        }
    }
}

impl<Q: Eq + ?Sized, K: Eq + Borrow<Q>, V> core::ops::Index<&Q> for Map<K, V> {
    type Output = V;

    fn index(&self, key: &Q) -> &V {
        self.get(key).expect("no entry found for key")
    }
}

pub enum Entry<'a, K: 'a, V: 'a> {
    Occupied(OccupiedEntry<'a, K, V>),
    Vacant(VacantEntry<'a, K, V>),
}

impl<'a, K, V> Entry<'a, K, V> {
    pub fn and_modify(mut self, f: impl FnOnce(&mut V)) -> Self {
        if let Entry::Occupied(oe) = &mut self {
            f(oe.get_mut())
        }
        self
    }

    pub fn key(&self) -> &K {
        match self {
            Entry::Occupied(oe) => oe.key(),
            Entry::Vacant(ve) => ve.key(),
        }
    }

    pub fn or_insert(self, default: V) -> &'a mut V {
        match self {
            Entry::Occupied(oe) => oe.into_mut(),
            Entry::Vacant(ve) => ve.insert(default),
        }
    }

    pub fn or_insert_with(self, f: impl FnOnce() -> V) -> &'a mut V {
        match self {
            Entry::Occupied(oe) => oe.into_mut(),
            Entry::Vacant(ve) => ve.insert(f()),
        }
    }
}

impl<'a, K: 'a, V: Default> Entry<'a, K, V> {
    pub fn or_default(self) -> &'a mut V {
        self.or_insert(Default::default())
    }
}

pub struct OccupiedEntry<'a, K: 'a, V: 'a> {
    entry_pos: usize,
    backing: &'a mut Vec<(K, V)>,
}

impl<'a, K: 'a, V: 'a> OccupiedEntry<'a, K, V> {
    pub fn get(&self) -> &V {
        &self.backing[self.entry_pos].1
    }

    pub fn get_mut(&mut self) -> &mut V {
        &mut self.backing[self.entry_pos].1
    }

    pub fn insert(&mut self, value: V) -> V {
        core::mem::replace(self.get_mut(), value)
    }

    pub fn into_mut(self) -> &'a mut V {
        &mut self.backing[self.entry_pos].1
    }

    pub fn key(&self) -> &K {
        &self.backing[self.entry_pos].0
    }

    pub fn remove(self) -> V {
        self.backing.remove(self.entry_pos).1
    }
}

pub struct VacantEntry<'a, K: 'a, V: 'a> {
    key: K,
    backing: &'a mut Vec<(K, V)>,
}

impl<'a, K: 'a, V: 'a> VacantEntry<'a, K, V> {
    pub fn insert(self, value: V) -> &'a mut V {
        self.backing.push((self.key, value));
        &mut self.backing.last_mut().unwrap().1
    }

    pub fn into_key(self) -> K {
        self.key
    }

    pub fn key(&self) -> &K {
        &self.key
    }
}

impl<K, V> BorshSerialize for Map<K, V>
where
    K: BorshSerialize + PartialOrd,
    V: BorshSerialize,
{
    fn serialize<W: std::io::Write>(&self, writer: &mut W) -> Result<(), std::io::Error> {
        let mut sorted = self.backing.iter().collect::<Vec<_>>();
        sorted.sort_by(|(a, _), (b, _)| (*a).partial_cmp(b).unwrap());
        (sorted.len() as u32).serialize(writer)?;
        for (key, value) in sorted {
            key.serialize(writer)?;
            value.serialize(writer)?;
        }
        Ok(())
    }
}

impl<K, V> BorshDeserialize for Map<K, V>
where
    K: BorshDeserialize + Eq,
    V: BorshDeserialize,
{
    fn deserialize<R: std::io::Read>(reader: &mut R) -> Result<Self, std::io::Error> {
        let len = u32::deserialize(reader)?;
        let mut backing: Vec<(K, V)> = Vec::with_capacity(len as usize);
        for i in 0..len {
            let key = K::deserialize(reader)?;
            let value = V::deserialize(reader)?;
            if i > 0 && key == backing.last().unwrap().0 {
                backing.last_mut().unwrap().1 = value;
            } else {
                backing.push((key, value));
            }
        }
        Ok(Self { backing })
    }
}

// taken from libstd/collections/hash/map.rs @ 7454b2
#[cfg(test)]
mod test_map {
    use super::*;
    use Entry::{Occupied, Vacant};

    use std::{cell::RefCell, collections::TryReserveError, usize};

    use rand::{thread_rng, Rng};

    #[test]
    fn test_zero_capacities() {
        type M = Map<i32, i32>;

        let m = M::new();
        assert_eq!(m.capacity(), 0);

        let m = M::default();
        assert_eq!(m.capacity(), 0);

        let m = M::with_capacity(0);
        assert_eq!(m.capacity(), 0);

        let mut m = M::new();
        m.insert(1, 1);
        m.insert(2, 2);
        m.remove(&1);
        m.remove(&2);
        m.shrink_to_fit();
        assert_eq!(m.capacity(), 0);

        let mut m = M::new();
        m.reserve(0);
        assert_eq!(m.capacity(), 0);
    }

    #[test]
    fn test_create_capacity_zero() {
        let mut m = Map::with_capacity(0);

        assert!(m.insert(1, 1).is_none());

        assert!(m.contains_key(&1));
        assert!(!m.contains_key(&0));
    }

    #[test]
    fn test_insert() {
        let mut m = Map::new();
        assert_eq!(m.len(), 0);
        assert!(m.insert(1, 2).is_none());
        assert_eq!(m.len(), 1);
        assert!(m.insert(2, 4).is_none());
        assert_eq!(m.len(), 2);
        assert_eq!(*m.get(&1).unwrap(), 2);
        assert_eq!(*m.get(&2).unwrap(), 4);
    }

    #[test]
    fn test_clone() {
        let mut m = Map::new();
        assert_eq!(m.len(), 0);
        assert!(m.insert(1, 2).is_none());
        assert_eq!(m.len(), 1);
        assert!(m.insert(2, 4).is_none());
        assert_eq!(m.len(), 2);
        let m2 = m.clone();
        assert_eq!(*m2.get(&1).unwrap(), 2);
        assert_eq!(*m2.get(&2).unwrap(), 4);
        assert_eq!(m2.len(), 2);
    }

    thread_local! { static DROP_VECTOR: RefCell<Vec<i32>> = RefCell::new(Vec::new()) }

    #[derive(PartialEq, Eq)]
    struct Droppable {
        k: usize,
    }

    impl Droppable {
        fn new(k: usize) -> Droppable {
            DROP_VECTOR.with(|slot| {
                slot.borrow_mut()[k] += 1;
            });

            Droppable { k }
        }
    }

    impl Drop for Droppable {
        fn drop(&mut self) {
            DROP_VECTOR.with(|slot| {
                slot.borrow_mut()[self.k] -= 1;
            });
        }
    }

    impl Clone for Droppable {
        fn clone(&self) -> Droppable {
            Droppable::new(self.k)
        }
    }

    #[test]
    fn test_drops() {
        DROP_VECTOR.with(|slot| {
            *slot.borrow_mut() = vec![0; 200];
        });

        {
            let mut m = Map::new();

            DROP_VECTOR.with(|v| {
                for i in 0..200 {
                    assert_eq!(v.borrow()[i], 0);
                }
            });

            for i in 0..100 {
                let d1 = Droppable::new(i);
                let d2 = Droppable::new(i + 100);
                m.insert(d1, d2);
            }

            DROP_VECTOR.with(|v| {
                for i in 0..200 {
                    assert_eq!(v.borrow()[i], 1);
                }
            });

            for i in 0..50 {
                let k = Droppable::new(i);
                let v = m.remove(&k);

                assert!(v.is_some());

                DROP_VECTOR.with(|v| {
                    assert_eq!(v.borrow()[i], 1);
                    assert_eq!(v.borrow()[i + 100], 1);
                });
            }

            DROP_VECTOR.with(|v| {
                for i in 0..50 {
                    assert_eq!(v.borrow()[i], 0);
                    assert_eq!(v.borrow()[i + 100], 0);
                }

                for i in 50..100 {
                    assert_eq!(v.borrow()[i], 1);
                    assert_eq!(v.borrow()[i + 100], 1);
                }
            });
        }

        DROP_VECTOR.with(|v| {
            for i in 0..200 {
                assert_eq!(v.borrow()[i], 0);
            }
        });
    }

    #[test]
    fn test_into_iter_drops() {
        DROP_VECTOR.with(|v| {
            *v.borrow_mut() = vec![0; 200];
        });

        let hm = {
            let mut hm = Map::new();

            DROP_VECTOR.with(|v| {
                for i in 0..200 {
                    assert_eq!(v.borrow()[i], 0);
                }
            });

            for i in 0..100 {
                let d1 = Droppable::new(i);
                let d2 = Droppable::new(i + 100);
                hm.insert(d1, d2);
            }

            DROP_VECTOR.with(|v| {
                for i in 0..200 {
                    assert_eq!(v.borrow()[i], 1);
                }
            });

            hm
        };

        // By the way, ensure that cloning doesn't screw up the dropping.
        drop(hm.clone());

        {
            let mut half = hm.into_iter().take(50);

            DROP_VECTOR.with(|v| {
                for i in 0..200 {
                    assert_eq!(v.borrow()[i], 1);
                }
            });

            for _ in half.by_ref() {}

            DROP_VECTOR.with(|v| {
                let nk = (0..100).filter(|&i| v.borrow()[i] == 1).count();

                let nv = (0..100).filter(|&i| v.borrow()[i + 100] == 1).count();

                assert_eq!(nk, 50);
                assert_eq!(nv, 50);
            });
        };

        DROP_VECTOR.with(|v| {
            for i in 0..200 {
                assert_eq!(v.borrow()[i], 0);
            }
        });
    }

    #[test]
    fn test_empty_remove() {
        let mut m: Map<i32, bool> = Map::new();
        assert_eq!(m.remove(&0), None);
    }

    #[test]
    fn test_empty_entry() {
        let mut m: Map<i32, bool> = Map::new();
        match m.entry(0) {
            Occupied(_) => panic!(),
            Vacant(_) => {}
        }
        assert!(*m.entry(0).or_insert(true));
        assert_eq!(m.len(), 1);
    }

    #[test]
    fn test_empty_iter() {
        let mut m: Map<i32, bool> = Map::new();
        assert_eq!(m.drain().next(), None);
        assert_eq!(m.keys().next(), None);
        assert_eq!(m.values().next(), None);
        assert_eq!(m.values_mut().next(), None);
        assert_eq!(m.iter().next(), None);
        assert_eq!(m.iter_mut().next(), None);
        assert_eq!(m.len(), 0);
        assert!(m.is_empty());
        assert_eq!(m.into_iter().next(), None);
    }

    // takes too long for non-fast map
    // #[test]
    // fn test_lots_of_insertions() {
    //     let mut m = Map::new();
    //
    //     // Try this a few times to make sure we never screw up the map's
    //     // internal state.
    //     for _ in 0..10 {
    //         assert!(m.is_empty());
    //
    //         for i in 1..1001 {
    //             assert!(m.insert(i, i).is_none());
    //
    //             for j in 1..=i {
    //                 let r = m.get(&j);
    //                 assert_eq!(r, Some(&j));
    //             }
    //
    //             for j in i + 1..1001 {
    //                 let r = m.get(&j);
    //                 assert_eq!(r, None);
    //             }
    //         }
    //
    //         for i in 1001..2001 {
    //             assert!(!m.contains_key(&i));
    //         }
    //
    //         // remove forwards
    //         for i in 1..1001 {
    //             assert!(m.remove(&i).is_some());
    //
    //             for j in 1..=i {
    //                 assert!(!m.contains_key(&j));
    //             }
    //
    //             for j in i + 1..1001 {
    //                 assert!(m.contains_key(&j));
    //             }
    //         }
    //
    //         for i in 1..1001 {
    //             assert!(!m.contains_key(&i));
    //         }
    //
    //         for i in 1..1001 {
    //             assert!(m.insert(i, i).is_none());
    //         }
    //
    //         // remove backwards
    //         for i in (1..1001).rev() {
    //             assert!(m.remove(&i).is_some());
    //
    //             for j in i..1001 {
    //                 assert!(!m.contains_key(&j));
    //             }
    //
    //             for j in 1..i {
    //                 assert!(m.contains_key(&j));
    //             }
    //         }
    //     }
    // }

    #[test]
    fn test_find_mut() {
        let mut m = Map::new();
        assert!(m.insert(1, 12).is_none());
        assert!(m.insert(2, 8).is_none());
        assert!(m.insert(5, 14).is_none());
        let new = 100;
        match m.get_mut(&5) {
            None => panic!(),
            Some(x) => *x = new,
        }
        assert_eq!(m.get(&5), Some(&new));
    }

    #[test]
    fn test_insert_overwrite() {
        let mut m = Map::new();
        assert!(m.insert(1, 2).is_none());
        assert_eq!(*m.get(&1).unwrap(), 2);
        assert!(!m.insert(1, 3).is_none());
        assert_eq!(*m.get(&1).unwrap(), 3);
    }

    #[test]
    fn test_insert_conflicts() {
        let mut m = Map::with_capacity(4);
        assert!(m.insert(1, 2).is_none());
        assert!(m.insert(5, 3).is_none());
        assert!(m.insert(9, 4).is_none());
        assert_eq!(*m.get(&9).unwrap(), 4);
        assert_eq!(*m.get(&5).unwrap(), 3);
        assert_eq!(*m.get(&1).unwrap(), 2);
    }

    #[test]
    fn test_conflict_remove() {
        let mut m = Map::with_capacity(4);
        assert!(m.insert(1, 2).is_none());
        assert_eq!(*m.get(&1).unwrap(), 2);
        assert!(m.insert(5, 3).is_none());
        assert_eq!(*m.get(&1).unwrap(), 2);
        assert_eq!(*m.get(&5).unwrap(), 3);
        assert!(m.insert(9, 4).is_none());
        assert_eq!(*m.get(&1).unwrap(), 2);
        assert_eq!(*m.get(&5).unwrap(), 3);
        assert_eq!(*m.get(&9).unwrap(), 4);
        assert!(m.remove(&1).is_some());
        assert_eq!(*m.get(&9).unwrap(), 4);
        assert_eq!(*m.get(&5).unwrap(), 3);
    }

    #[test]
    fn test_is_empty() {
        let mut m = Map::with_capacity(4);
        assert!(m.insert(1, 2).is_none());
        assert!(!m.is_empty());
        assert!(m.remove(&1).is_some());
        assert!(m.is_empty());
    }

    #[test]
    fn test_remove() {
        let mut m = Map::new();
        m.insert(1, 2);
        assert_eq!(m.remove(&1), Some(2));
        assert_eq!(m.remove(&1), None);
    }

    #[test]
    fn test_remove_entry() {
        let mut m = Map::new();
        m.insert(1, 2);
        assert_eq!(m.remove_entry(&1), Some((1, 2)));
        assert_eq!(m.remove(&1), None);
    }

    #[test]
    fn test_iterate() {
        let mut m = Map::with_capacity(4);
        for i in 0..32 {
            assert!(m.insert(i, i * 2).is_none());
        }
        assert_eq!(m.len(), 32);

        let mut observed: u32 = 0;

        for (k, v) in &m {
            assert_eq!(*v, *k * 2);
            observed |= 1 << *k;
        }
        assert_eq!(observed, 0xFFFF_FFFF);
    }

    #[test]
    fn test_keys() {
        let vec = vec![(1, 'a'), (2, 'b'), (3, 'c')];
        let map: Map<_, _> = vec.into_iter().collect();
        let keys: Vec<_> = map.keys().cloned().collect();
        assert_eq!(keys.len(), 3);
        assert!(keys.contains(&1));
        assert!(keys.contains(&2));
        assert!(keys.contains(&3));
    }

    #[test]
    fn test_values() {
        let vec = vec![(1, 'a'), (2, 'b'), (3, 'c')];
        let map: Map<_, _> = vec.into_iter().collect();
        let values: Vec<_> = map.values().cloned().collect();
        assert_eq!(values.len(), 3);
        assert!(values.contains(&'a'));
        assert!(values.contains(&'b'));
        assert!(values.contains(&'c'));
    }

    #[test]
    fn test_values_mut() {
        let vec = vec![(1, 1), (2, 2), (3, 3)];
        let mut map: Map<_, _> = vec.into_iter().collect();
        for value in map.values_mut() {
            *value = (*value) * 2
        }
        let values: Vec<_> = map.values().cloned().collect();
        assert_eq!(values.len(), 3);
        assert!(values.contains(&2));
        assert!(values.contains(&4));
        assert!(values.contains(&6));
    }

    #[test]
    fn test_find() {
        let mut m = Map::new();
        assert!(m.get(&1).is_none());
        m.insert(1, 2);
        match m.get(&1) {
            None => panic!(),
            Some(v) => assert_eq!(*v, 2),
        }
    }

    #[test]
    fn test_eq() {
        let mut m1 = Map::new();
        m1.insert(1, 2);
        m1.insert(2, 3);
        m1.insert(3, 4);

        let mut m2 = Map::new();
        m2.insert(1, 2);
        m2.insert(2, 3);

        assert!(m1 != m2);

        m2.insert(3, 4);

        assert_eq!(m1, m2);
    }

    #[test]
    fn test_show() {
        let mut map = Map::new();
        let empty: Map<i32, i32> = Map::new();

        map.insert(1, 2);
        map.insert(3, 4);

        let map_str = format!("{:?}", map);

        assert!(map_str == "{1: 2, 3: 4}" || map_str == "{3: 4, 1: 2}");
        assert_eq!(format!("{:?}", empty), "{}");
    }

    #[test]
    fn test_reserve_shrink_to_fit() {
        let mut m = Map::new();
        m.insert(0, 0);
        m.remove(&0);
        assert!(m.capacity() >= m.len());
        for i in 0..128 {
            m.insert(i, i);
        }
        m.reserve(256);

        let usable_cap = m.capacity();
        for i in 128..(128 + 256) {
            m.insert(i, i);
            assert_eq!(m.capacity(), usable_cap);
        }

        for i in 100..(128 + 256) {
            assert_eq!(m.remove(&i), Some(i));
        }
        m.shrink_to_fit();

        assert_eq!(m.len(), 100);
        assert!(!m.is_empty());
        assert!(m.capacity() >= m.len());

        for i in 0..100 {
            assert_eq!(m.remove(&i), Some(i));
        }
        m.shrink_to_fit();
        m.insert(0, 0);

        assert_eq!(m.len(), 1);
        assert!(m.capacity() >= m.len());
        assert_eq!(m.remove(&0), Some(0));
    }

    #[test]
    fn test_from_iter() {
        let xs = [(1, 1), (2, 2), (2, 2), (3, 3), (4, 4), (5, 5), (6, 6)];

        let map: Map<_, _> = xs.iter().cloned().collect();

        for &(k, v) in &xs {
            assert_eq!(map.get(&k), Some(&v));
        }

        assert_eq!(map.iter().len(), xs.len() - 1);
    }

    #[test]
    fn test_size_hint() {
        let xs = [(1, 1), (2, 2), (3, 3), (4, 4), (5, 5), (6, 6)];

        let map: Map<_, _> = xs.iter().cloned().collect();

        let mut iter = map.iter();

        for _ in iter.by_ref().take(3) {}

        assert_eq!(iter.size_hint(), (3, Some(3)));
    }

    #[test]
    fn test_iter_len() {
        let xs = [(1, 1), (2, 2), (3, 3), (4, 4), (5, 5), (6, 6)];

        let map: Map<_, _> = xs.iter().cloned().collect();

        let mut iter = map.iter();

        for _ in iter.by_ref().take(3) {}

        assert_eq!(iter.len(), 3);
    }

    #[test]
    fn test_mut_size_hint() {
        let xs = [(1, 1), (2, 2), (3, 3), (4, 4), (5, 5), (6, 6)];

        let mut map: Map<_, _> = xs.iter().cloned().collect();

        let mut iter = map.iter_mut();

        for _ in iter.by_ref().take(3) {}

        assert_eq!(iter.size_hint(), (3, Some(3)));
    }

    #[test]
    fn test_iter_mut_len() {
        let xs = [(1, 1), (2, 2), (3, 3), (4, 4), (5, 5), (6, 6)];

        let mut map: Map<_, _> = xs.iter().cloned().collect();

        let mut iter = map.iter_mut();

        for _ in iter.by_ref().take(3) {}

        assert_eq!(iter.len(), 3);
    }

    #[test]
    fn test_index() {
        let mut map = Map::new();

        map.insert(1, 2);
        map.insert(2, 1);
        map.insert(3, 4);

        assert_eq!(map[&2], 1);
    }

    #[test]
    #[should_panic]
    fn test_index_nonexistent() {
        let mut map = Map::new();

        map.insert(1, 2);
        map.insert(2, 1);
        map.insert(3, 4);

        map[&4];
    }

    #[test]
    fn test_entry() {
        let xs = [(1, 10), (2, 20), (3, 30), (4, 40), (5, 50), (6, 60)];

        let mut map: Map<_, _> = xs.iter().cloned().collect();

        // Existing key (insert)
        match map.entry(1) {
            Vacant(_) => unreachable!(),
            Occupied(mut view) => {
                assert_eq!(view.get(), &10);
                assert_eq!(view.insert(100), 10);
            }
        }
        assert_eq!(map.get(&1).unwrap(), &100);
        assert_eq!(map.len(), 6);

        // Existing key (update)
        match map.entry(2) {
            Vacant(_) => unreachable!(),
            Occupied(mut view) => {
                let v = view.get_mut();
                let new_v = (*v) * 10;
                *v = new_v;
            }
        }
        assert_eq!(map.get(&2).unwrap(), &200);
        assert_eq!(map.len(), 6);

        // Existing key (take)
        match map.entry(3) {
            Vacant(_) => unreachable!(),
            Occupied(view) => {
                assert_eq!(view.remove(), 30);
            }
        }
        assert_eq!(map.get(&3), None);
        assert_eq!(map.len(), 5);

        // Inexistent key (insert)
        match map.entry(10) {
            Occupied(_) => unreachable!(),
            Vacant(view) => {
                assert_eq!(*view.insert(1000), 1000);
            }
        }
        assert_eq!(map.get(&10).unwrap(), &1000);
        assert_eq!(map.len(), 6);
    }

    #[test]
    fn test_entry_take_doesnt_corrupt() {
        #![allow(deprecated)] //rand
                              // Test for #19292
        fn check(m: &Map<i32, ()>) {
            for k in m.keys() {
                assert!(m.contains_key(k), "{} is in keys() but not in the map?", k);
            }
        }

        let mut m = Map::new();
        let mut rng = thread_rng();

        // Populate the map with some items.
        for _ in 0..50 {
            let x = rng.gen_range(-10, 10);
            m.insert(x, ());
        }

        for _ in 0..1000 {
            let x = rng.gen_range(-10, 10);
            match m.entry(x) {
                Vacant(_) => {}
                Occupied(e) => {
                    e.remove();
                }
            }

            check(&m);
        }
    }

    #[test]
    fn test_extend_ref() {
        let mut a = Map::new();
        a.insert(1, "one");
        let mut b = Map::new();
        b.insert(2, "two");
        b.insert(3, "three");

        a.extend(&b);

        assert_eq!(a.len(), 3);
        assert_eq!(a[&1], "one");
        assert_eq!(a[&2], "two");
        assert_eq!(a[&3], "three");
    }

    #[test]
    fn test_capacity_not_less_than_len() {
        let mut a = Map::new();
        let mut item = 0;

        for _ in 0..116 {
            a.insert(item, 0);
            item += 1;
        }

        assert!(a.capacity() > a.len());

        let free = a.capacity() - a.len();
        for _ in 0..free {
            a.insert(item, 0);
            item += 1;
        }

        assert_eq!(a.len(), a.capacity());

        // Insert at capacity should cause allocation.
        a.insert(item, 0);
        assert!(a.capacity() > a.len());
    }

    #[test]
    fn test_occupied_entry_key() {
        let mut a = Map::new();
        let key = "hello there";
        let value = "value goes here";
        assert!(a.is_empty());
        a.insert(key.clone(), value.clone());
        assert_eq!(a.len(), 1);
        assert_eq!(a[key], value);

        match a.entry(key.clone()) {
            Vacant(_) => panic!(),
            Occupied(e) => assert_eq!(key, *e.key()),
        }
        assert_eq!(a.len(), 1);
        assert_eq!(a[key], value);
    }

    #[test]
    fn test_vacant_entry_key() {
        let mut a = Map::new();
        let key = "hello there";
        let value = "value goes here";

        assert!(a.is_empty());
        match a.entry(key.clone()) {
            Occupied(_) => panic!(),
            Vacant(e) => {
                assert_eq!(key, *e.key());
                e.insert(value.clone());
            }
        }
        assert_eq!(a.len(), 1);
        assert_eq!(a[key], value);
    }

    #[test]
    fn test_retain() {
        let mut map: Map<i32, i32> = (0..100).map(|x| (x, x * 10)).collect();

        map.retain(|&k, _| k % 2 == 0);
        assert_eq!(map.len(), 50);
        assert_eq!(map[&2], 20);
        assert_eq!(map[&4], 40);
        assert_eq!(map[&6], 60);
    }

    #[test]
    fn test_try_reserve() {
        let mut empty_bytes: Map<u8, u8> = Map::new();

        const MAX_USIZE: usize = usize::MAX;

        if let Err(TryReserveError::CapacityOverflow) = empty_bytes.try_reserve(MAX_USIZE) {
        } else {
            panic!("usize::MAX should trigger an overflow!");
        }

        if let Err(TryReserveError::AllocError { .. }) = empty_bytes.try_reserve(MAX_USIZE / 8) {
        } else {
            panic!("usize::MAX / 8 should trigger an OOM!")
        }
    }

    #[test]
    fn test_borsh_roundtrip() {
        let mut m = Map::new();
        m.insert("onefish".to_string(), "twofish".to_string());
        m.insert("redfish".to_string(), "bluefish".to_string());
        let m2: Map<String, String> =
            BorshDeserialize::try_from_slice(&m.try_to_vec().unwrap()).unwrap();
        assert_eq!(m2, m);
    }

    #[test]
    fn test_borsh_nonunique() {
        let mut not_map = Vec::new();
        not_map.push(("a".to_string(), "a".to_string()));
        not_map.push(("a".to_string(), "b".to_string()));
        not_map.push(("b".to_string(), "b".to_string()));
        let m2: Map<String, String> =
            BorshDeserialize::try_from_slice(&not_map.try_to_vec().unwrap()).unwrap();
        assert_eq!(m2, not_map.iter().cloned().collect());
    }
}
