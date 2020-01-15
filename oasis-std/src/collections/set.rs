use std::{
    borrow::Borrow,
    fmt::{self, Debug},
};

use oasis_borsh::{BorshDeserialize, BorshSerialize};

/// `Set` is a data structure with a [`Set`](https://doc.rust-lang.org/std/collections/hash_set/struct.HashSet.html)-like API but based on a `Vec`.
/// It's primarily useful when you care about constant factors or prefer determinism to speed.
/// Please refer to the [docs for `Set`](https://doc.rust-lang.org/std/collections/hash_set/struct.HashSet.html) for details and examples of the Set API.
///
/// ## Example
///
/// ```
/// use oasis_std::collections::Set;
/// let mut set1 = Set::new();
/// let mut set2 = Set::new();
/// set1.insert(1);
/// set1.insert(2);
/// set2.insert(2);
/// set2.insert(3);
/// let mut set3 = Set::with_capacity(1);
/// assert!(set3.insert(3));
/// assert_eq!(&set2 - &set1, set3);
/// ```
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Set<T> {
    backing: Vec<T>,
}

impl<T: Eq> Set<T> {
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

    pub fn contains<Q: ?Sized>(&self, value: &Q) -> bool
    where
        T: Borrow<Q>,
        Q: Eq,
    {
        self.backing.iter().any(|v| value.eq(v.borrow()))
    }

    pub fn difference<'a>(
        &'a self,
        other: &'a Self,
    ) -> impl Iterator<Item = &'a T> + DoubleEndedIterator {
        self.backing.iter().filter(move |v| !other.contains(v))
    }

    pub fn drain(&mut self) -> std::vec::Drain<T> {
        self.backing.drain(..)
    }

    pub fn get<Q: ?Sized>(&self, value: &Q) -> Option<&T>
    where
        T: Borrow<Q>,
        Q: Eq,
    {
        self.backing.iter().find(|v| value.eq((*v).borrow()))
    }

    pub fn get_or_insert(&mut self, value: T) -> &T {
        let self_ptr = self as *mut Self;
        for v in self.backing.iter() {
            if *v == value {
                return v;
            }
        }
        // rustc just isn't having it
        unsafe { (*self_ptr).backing.push(value) };
        self.backing.last().unwrap()
    }

    pub fn get_or_insert_with<Q: ?Sized>(&mut self, value: &Q, f: impl FnOnce(&Q) -> T) -> &T
    where
        T: Borrow<Q>,
        Q: Eq,
    {
        let self_ptr = self as *mut Self;
        for v in self.backing.iter() {
            if (*v).borrow() == value {
                return v;
            }
        }
        unsafe { (*self_ptr).backing.push(f(value)) };
        self.backing.last().unwrap()
    }

    pub fn insert(&mut self, value: T) -> bool {
        !self.backing.iter().any(|v| *v == value) && {
            self.backing.push(value);
            true
        }
    }

    pub fn intersection<'a>(
        &'a self,
        other: &'a Self,
    ) -> impl Iterator<Item = &'a T> + DoubleEndedIterator<Item = &'a T> {
        self.backing.iter().filter(move |v| other.contains(v))
    }

    pub fn is_disjoint<'a>(&'a self, other: &'a Self) -> bool {
        self.intersection(other).count() == 0
    }

    pub fn is_empty(&self) -> bool {
        self.backing.is_empty()
    }

    pub fn is_subset(&self, other: &Self) -> bool {
        self.len() <= other.len() && self.difference(other).count() == 0
    }

    pub fn is_superset(&self, other: &Self) -> bool {
        other.is_subset(self)
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> + DoubleEndedIterator + ExactSizeIterator {
        self.backing.iter()
    }

    pub fn len(&self) -> usize {
        self.backing.len()
    }

    pub fn remove<Q: ?Sized>(&mut self, value: &Q) -> bool
    where
        T: Borrow<Q>,
        Q: Eq,
    {
        self.take(value).is_some()
    }

    pub fn replace(&mut self, value: T) -> Option<T> {
        match self.backing.iter_mut().find(|v| **v == value) {
            Some(v) => Some(core::mem::replace(v, value)),
            None => {
                self.backing.push(value);
                None
            }
        }
    }

    pub fn reserve(&mut self, additional: usize) {
        self.backing.reserve(additional)
    }

    pub fn retain(&mut self, mut f: impl FnMut(&T) -> bool) {
        self.backing.drain_filter(|v| !f(v));
    }

    pub fn shrink_to(&mut self, min_capacity: usize) {
        self.backing.shrink_to(min_capacity)
    }

    pub fn shrink_to_fit(&mut self) {
        self.backing.shrink_to_fit()
    }

    pub fn symmetric_difference<'a>(
        &'a self,
        other: &'a Self,
    ) -> impl Iterator<Item = &'a T> + DoubleEndedIterator {
        self.difference(other).chain(other.difference(self))
    }

    pub fn take<Q: ?Sized>(&mut self, value: &Q) -> Option<T>
    where
        T: Borrow<Q>,
        Q: Eq,
    {
        self.backing
            .iter()
            .position(|v| value.eq(v.borrow()))
            .map(|pos| self.backing.remove(pos))
    }

    pub fn try_reserve(
        &mut self,
        additional: usize,
    ) -> Result<(), std::collections::TryReserveError> {
        self.backing.try_reserve(additional)
    }

    pub fn union<'a>(
        &'a self,
        other: &'a Self,
    ) -> impl Iterator<Item = &'a T> + DoubleEndedIterator {
        self.iter().chain(other.difference(self))
    }
}

impl<T: Debug> fmt::Debug for Set<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.backing.iter()).finish()
    }
}

impl<'a, T> IntoIterator for &'a Set<T> {
    type Item = &'a T;
    type IntoIter = core::slice::Iter<'a, T>;

    fn into_iter(self) -> <Self as IntoIterator>::IntoIter {
        self.backing.iter()
    }
}

impl<T> IntoIterator for Set<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;

    fn into_iter(self) -> <Self as IntoIterator>::IntoIter {
        self.backing.into_iter()
    }
}

impl<T: Eq> core::iter::FromIterator<T> for Set<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut this = Self::new();
        this.extend(iter);
        this
    }
}

impl<T: Eq> Extend<T> for Set<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for item in iter {
            self.insert(item);
        }
    }
}

impl<'a, T: 'a + Copy + Eq> Extend<&'a T> for Set<T> {
    fn extend<I: IntoIterator<Item = &'a T>>(&mut self, iter: I) {
        for item in iter {
            self.insert(*item);
        }
    }
}

impl<T: Clone + Eq> core::ops::BitOr<&Set<T>> for &Set<T> {
    type Output = Set<T>;
    fn bitor(self, rhs: &Set<T>) -> Set<T> {
        self.union(rhs).cloned().collect()
    }
}

impl<T: Clone + Eq> core::ops::BitAnd<&Set<T>> for &Set<T> {
    type Output = Set<T>;
    fn bitand(self, rhs: &Set<T>) -> Set<T> {
        self.intersection(rhs).cloned().collect()
    }
}

impl<T: Clone + Eq> core::ops::BitXor<&Set<T>> for &Set<T> {
    type Output = Set<T>;
    fn bitxor(self, rhs: &Set<T>) -> Set<T> {
        self.symmetric_difference(rhs).cloned().collect()
    }
}

impl<T: Clone + Eq> core::ops::Sub<&Set<T>> for &Set<T> {
    type Output = Set<T>;
    fn sub(self, rhs: &Set<T>) -> Set<T> {
        self.difference(rhs).cloned().collect()
    }
}

impl<T> BorshSerialize for Set<T>
where
    T: BorshSerialize + PartialOrd,
{
    fn serialize<W: std::io::Write>(&self, writer: &mut W) -> Result<(), std::io::Error> {
        let mut sorted = self.backing.iter().collect::<Vec<_>>();
        sorted.sort_by(|a, b| (*a).partial_cmp(b).unwrap());
        (sorted.len() as u32).serialize(writer)?;
        for item in sorted {
            item.serialize(writer)?;
        }
        Ok(())
    }
}

impl<T> BorshDeserialize for Set<T>
where
    T: BorshDeserialize + Eq,
{
    fn deserialize<R: std::io::Read>(reader: &mut R) -> Result<Self, std::io::Error> {
        let len = u32::deserialize(reader)?;
        let mut backing: Vec<T> = Vec::with_capacity(len as usize);
        for _ in 0..len {
            let item = T::deserialize(reader)?;
            if backing.last().map(|prev| item == *prev).unwrap_or_default() {
                continue;
            }
            backing.push(item);
        }
        Ok(Self { backing })
    }
}

// taken from libstd/collections/hash/set.rs @ 7454b2
#[cfg(test)]
#[allow(clippy::all)]
mod test_set {
    use super::*;

    #[test]
    fn test_zero_capacities() {
        type S = Set<i32>;

        let s = S::new();
        assert_eq!(s.capacity(), 0);

        let s = S::default();
        assert_eq!(s.capacity(), 0);

        let s = S::with_capacity(0);
        assert_eq!(s.capacity(), 0);

        let mut s = S::new();
        s.insert(1);
        s.insert(2);
        s.remove(&1);
        s.remove(&2);
        s.shrink_to_fit();
        assert_eq!(s.capacity(), 0);

        let mut s = S::new();
        s.reserve(0);
        assert_eq!(s.capacity(), 0);
    }

    #[test]
    fn test_disjoint() {
        let mut xs = Set::new();
        let mut ys = Set::new();
        assert!(xs.is_disjoint(&ys));
        assert!(ys.is_disjoint(&xs));
        assert!(xs.insert(5));
        assert!(ys.insert(11));
        assert!(xs.is_disjoint(&ys));
        assert!(ys.is_disjoint(&xs));
        assert!(xs.insert(7));
        assert!(xs.insert(19));
        assert!(xs.insert(4));
        assert!(ys.insert(2));
        assert!(ys.insert(-11));
        assert!(xs.is_disjoint(&ys));
        assert!(ys.is_disjoint(&xs));
        assert!(ys.insert(7));
        assert!(!xs.is_disjoint(&ys));
        assert!(!ys.is_disjoint(&xs));
    }

    #[test]
    fn test_subset_and_superset() {
        let mut a = Set::new();
        assert!(a.insert(0));
        assert!(a.insert(5));
        assert!(a.insert(11));
        assert!(a.insert(7));

        let mut b = Set::new();
        assert!(b.insert(0));
        assert!(b.insert(7));
        assert!(b.insert(19));
        assert!(b.insert(250));
        assert!(b.insert(11));
        assert!(b.insert(200));

        assert!(!a.is_subset(&b));
        assert!(!a.is_superset(&b));
        assert!(!b.is_subset(&a));
        assert!(!b.is_superset(&a));

        assert!(b.insert(5));

        assert!(a.is_subset(&b));
        assert!(!a.is_superset(&b));
        assert!(!b.is_subset(&a));
        assert!(b.is_superset(&a));
    }

    #[test]
    fn test_iterate() {
        let mut a = Set::new();
        for i in 0..32 {
            assert!(a.insert(i));
        }
        let mut observed: u32 = 0;
        for k in &a {
            observed |= 1 << *k;
        }
        assert_eq!(observed, 0xFFFF_FFFF);
    }

    #[test]
    fn test_intersection() {
        let mut a = Set::new();
        let mut b = Set::new();
        assert!(a.intersection(&b).next().is_none());

        assert!(a.insert(11));
        assert!(a.insert(1));
        assert!(a.insert(3));
        assert!(a.insert(77));
        assert!(a.insert(103));
        assert!(a.insert(5));
        assert!(a.insert(-5));

        assert!(b.insert(2));
        assert!(b.insert(11));
        assert!(b.insert(77));
        assert!(b.insert(-9));
        assert!(b.insert(-42));
        assert!(b.insert(5));
        assert!(b.insert(3));

        let mut i = 0;
        let expected = [3, 5, 11, 77];
        for x in a.intersection(&b) {
            assert!(expected.contains(x));
            i += 1
        }
        assert_eq!(i, expected.len());

        assert!(a.insert(9)); // make a bigger than b

        i = 0;
        for x in a.intersection(&b) {
            assert!(expected.contains(x));
            i += 1
        }
        assert_eq!(i, expected.len());

        i = 0;
        for x in b.intersection(&a) {
            assert!(expected.contains(x));
            i += 1
        }
        assert_eq!(i, expected.len());
    }

    #[test]
    fn test_difference() {
        let mut a = Set::new();
        let mut b = Set::new();

        assert!(a.insert(1));
        assert!(a.insert(3));
        assert!(a.insert(5));
        assert!(a.insert(9));
        assert!(a.insert(11));

        assert!(b.insert(3));
        assert!(b.insert(9));

        let mut i = 0;
        let expected = [1, 5, 11];
        for x in a.difference(&b) {
            assert!(expected.contains(x));
            i += 1
        }
        assert_eq!(i, expected.len());
    }

    #[test]
    fn test_symmetric_difference() {
        let mut a = Set::new();
        let mut b = Set::new();

        assert!(a.insert(1));
        assert!(a.insert(3));
        assert!(a.insert(5));
        assert!(a.insert(9));
        assert!(a.insert(11));

        assert!(b.insert(-2));
        assert!(b.insert(3));
        assert!(b.insert(9));
        assert!(b.insert(14));
        assert!(b.insert(22));

        let mut i = 0;
        let expected = [-2, 1, 5, 11, 14, 22];
        for x in a.symmetric_difference(&b) {
            assert!(expected.contains(x));
            i += 1
        }
        assert_eq!(i, expected.len());
    }

    #[test]
    fn test_union() {
        let mut a = Set::new();
        let mut b = Set::new();
        assert!(a.union(&b).next().is_none());
        assert!(b.union(&a).next().is_none());

        assert!(a.insert(1));
        assert!(a.insert(3));
        assert!(a.insert(11));
        assert!(a.insert(16));
        assert!(a.insert(19));
        assert!(a.insert(24));

        assert!(b.insert(-2));
        assert!(b.insert(1));
        assert!(b.insert(5));
        assert!(b.insert(9));
        assert!(b.insert(13));
        assert!(b.insert(19));

        let mut i = 0;
        let expected = [-2, 1, 3, 5, 9, 11, 13, 16, 19, 24];
        for x in a.union(&b) {
            assert!(expected.contains(x));
            i += 1
        }
        assert_eq!(i, expected.len());

        assert!(a.insert(9)); // make a bigger than b
        assert!(a.insert(5));

        i = 0;
        for x in a.union(&b) {
            assert!(expected.contains(x));
            i += 1
        }
        assert_eq!(i, expected.len());

        i = 0;
        for x in b.union(&a) {
            assert!(expected.contains(x));
            i += 1
        }
        assert_eq!(i, expected.len());
    }

    #[test]
    fn test_from_iter() {
        let xs = [1, 2, 2, 3, 4, 5, 6, 7, 8, 9];

        let set: Set<_> = xs.iter().cloned().collect();

        for x in &xs {
            assert!(set.contains(x));
        }

        assert_eq!(set.iter().len(), xs.len() - 1);
    }

    #[test]
    fn test_move_iter() {
        let hs = {
            let mut hs = Set::new();

            hs.insert('a');
            hs.insert('b');

            hs
        };

        let v = hs.into_iter().collect::<Vec<char>>();
        assert!(v == ['a', 'b'] || v == ['b', 'a']);
    }

    #[test]
    fn test_eq() {
        // These constants once happened to expose a bug in insert().
        // I'm keeping them around to prevent a regression.
        let mut s1 = Set::new();

        s1.insert(1);
        s1.insert(2);
        s1.insert(3);

        let mut s2 = Set::new();

        s2.insert(1);
        s2.insert(2);

        assert!(s1 != s2);

        s2.insert(3);

        assert_eq!(s1, s2);
    }

    #[test]
    fn test_show() {
        let mut set = Set::new();
        let empty = Set::<i32>::new();

        set.insert(1);
        set.insert(2);

        let set_str = format!("{:?}", set);

        assert!(set_str == "{1, 2}" || set_str == "{2, 1}");
        assert_eq!(format!("{:?}", empty), "{}");
    }

    #[test]
    fn test_trivial_drain() {
        let mut s = Set::<i32>::new();
        for _ in s.drain() {}
        assert!(s.is_empty());
        drop(s);

        let mut s = Set::<i32>::new();
        drop(s.drain());
        assert!(s.is_empty());
    }

    #[test]
    fn test_drain() {
        let mut s: Set<_> = (1..100).collect();

        // try this a bunch of times to make sure we don't screw up internal state.
        for _ in 0..20 {
            assert_eq!(s.len(), 99);

            {
                let mut last_i = 0;
                let mut d = s.drain();
                for (i, x) in d.by_ref().take(50).enumerate() {
                    last_i = i;
                    assert!(x != 0);
                }
                assert_eq!(last_i, 49);
            }

            for _ in &s {
                panic!("s should be empty!");
            }

            // reset to try again.
            s.extend(1..100);
        }
    }

    #[test]
    fn test_replace() {
        #[derive(Debug)]
        struct Foo(&'static str, i32);

        impl PartialEq for Foo {
            fn eq(&self, other: &Self) -> bool {
                self.0 == other.0
            }
        }

        impl Eq for Foo {}

        let mut s = Set::new();
        assert_eq!(s.replace(Foo("a", 1)), None);
        assert_eq!(s.len(), 1);
        assert_eq!(s.replace(Foo("a", 2)), Some(Foo("a", 1)));
        assert_eq!(s.len(), 1);

        let mut it = s.iter();
        assert_eq!(it.next(), Some(&Foo("a", 2)));
        assert_eq!(it.next(), None);
    }

    #[test]
    fn test_extend_ref() {
        let mut a = Set::new();
        a.insert(1);

        a.extend(&[2, 3, 4]);

        assert_eq!(a.len(), 4);
        assert!(a.contains(&1));
        assert!(a.contains(&2));
        assert!(a.contains(&3));
        assert!(a.contains(&4));

        let mut b = Set::new();
        b.insert(5);
        b.insert(6);

        a.extend(&b);

        assert_eq!(a.len(), 6);
        assert!(a.contains(&1));
        assert!(a.contains(&2));
        assert!(a.contains(&3));
        assert!(a.contains(&4));
        assert!(a.contains(&5));
        assert!(a.contains(&6));
    }

    #[test]
    fn test_retain() {
        let xs = [1, 2, 3, 4, 5, 6];
        let mut set: Set<i32> = xs.iter().cloned().collect();
        set.retain(|&k| k % 2 == 0);
        assert_eq!(set.len(), 3);
        assert!(set.contains(&2));
        assert!(set.contains(&4));
        assert!(set.contains(&6));
    }

    #[test]
    fn test_borsh_roundtrip() {
        let mut s = Set::new();
        s.insert("that".to_string());
        s.insert("the other thing".to_string());
        s.insert("this".to_string());
        let s2: Set<String> = BorshDeserialize::try_from_slice(&s.try_to_vec().unwrap()).unwrap();
        assert_eq!(s2, s);
    }

    #[test]
    fn test_borsh_nonunique() {
        let mut not_set = Vec::new();
        not_set.push("a".to_string());
        not_set.push("a".to_string());
        not_set.push("b".to_string());
        let s2: Set<String> =
            BorshDeserialize::try_from_slice(&not_set.try_to_vec().unwrap()).unwrap();
        assert_eq!(s2, not_set.iter().cloned().collect());
    }
}
