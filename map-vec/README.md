# map-vec

**The Map and Set APIs backed by Vec**
Vec-backed maps and sets are useful when you have small, short-lived data and care about constant factors.
Please refer to the rustdocs for HashMap and HashSet for details on and examples of using the Map/Set API.

[`map_vec::Map`](https://docs.rs/map-vec/latest/map-vec/map/struct.Map.html) is a data structure with the interface of [`HashMap`](https://doc.rust-lang.org/std/collections/hash_map/struct.HashMap.html).
Similarly [`map_vec::Set`](https://docs.rs/map_vec/latest/map_vec/set/struct.Set.html) is a data structure with the interface of [`HashSet`](https://doc.rust-lang.org/std/collections/hash_set/struct.HashSet.html).

Note: `Map` and `Set` are (de)serializable using [borsh](https://github.com/nearprotocol/borsh).

## Map Example

```rust
fn main() {
  let mut map = map_vec::Map::new();
  map.insert("hello".to_string(), "world".to_string());
  map.entry("hello".to_string()).and_modify(|mut v| v.push_str("!"));
  assert_eq!(map.get("hello").map(String::as_str), Some("world!"))
}
```

## Set Example

```rust
fn main() {
  let mut set1 = map_vec::Set::new();
  let mut set2 = map_vec::Set::new();
  set1.insert(1);
  set1.insert(2);
  set2.insert(2);
  set2.insert(3);
  let mut set3 = map_vec::Set::with_capacity(1);
  assert!(set3.insert(3));
  assert_eq!(&set2 - &set1, set3);
}
```
