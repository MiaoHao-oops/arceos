use hashbrown::hash_map as base;

#[allow(deprecated)]
use core::hash::{BuildHasher, Hash, SipHasher13, Hasher};
use core::default::Default;
use arceos_api::random;

pub struct HashMap<K, V, S = RandomState> {
    base: base::HashMap<K, V, S>,
}

impl<K: Eq + Hash, V> HashMap<K, V, RandomState> {
    /// 创建一个空的 `HashMap`。
    ///
    /// 哈希 map 最初创建时的容量为 0，因此只有在首次插入时才分配。
    ///
    ///
    /// # Examples
    ///
    /// ```
    /// use std::collections::HashMap;
    /// let mut map: HashMap<&str, i32> = HashMap::new();
    /// ```
    #[inline]
    #[must_use]
    pub fn new() -> HashMap<K, V, RandomState> {
        Default::default()
    }

    /// 迭代器元素类型为 `(&'a K, &'a V)`。
    ///
    /// # Examples
    ///
    /// ```
    /// use std::collections::HashMap;
    ///
    /// let map = HashMap::from([
    ///     ("a", 1),
    ///     ("b", 2),
    ///     ("c", 3),
    /// ]);
    ///
    /// for (key, val) in map.iter() {
    ///     println!("key: {key} val: {val}");
    /// }
    /// ```
    ///
    /// # Performance
    ///
    /// 在当前实现中，迭代 map 需要 O(capacity) 时间而不是 O(len) 时间，因为它在内部也访问了空的 buckets。
    ///
    pub fn iter(&self) -> Iter<'_, K, V> {
        Iter { base: self.base.iter() }
    }
}

impl<K, V, S> HashMap<K, V, S>
where
    K: Eq + Hash,
    S: BuildHasher,
{
    /// Creates an empty `HashMap` which will use the given hash builder to hash
    /// keys.
    ///
    /// The created map has the default initial capacity.
    ///
    /// Warning: `hash_builder` is normally randomly generated, and
    /// is designed to allow HashMaps to be resistant to attacks that
    /// cause many collisions and very poor performance. Setting it
    /// manually using this function can expose a DoS attack vector.
    ///
    /// The `hash_builder` passed should implement the [`BuildHasher`] trait for
    /// the HashMap to be useful, see its documentation for details.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::collections::HashMap;
    /// use std::collections::hash_map::RandomState;
    ///
    /// let s = RandomState::new();
    /// let mut map = HashMap::with_hasher(s);
    /// map.insert(1, 2);
    /// ```
    #[inline]
    pub const fn with_hasher(hash_builder: S) -> HashMap<K, V, S> {
        HashMap { base: base::HashMap::with_hasher(hash_builder) }
    }
    /// 将键值对插入 map。
    ///
    /// 如果 map 不存在此键，则返回 [`None`]。
    ///
    /// 如果 map 确实存在此键，则更新值，并返回旧值。
    /// 但是，键不会更新。对于不能相同的 `==` 类型来说，这一点很重要。
    ///
    /// 有关更多信息，请参见 [模块级文档][module-level documentation]。
    ///
    /// [module-level documentation]: crate::collections#insert-and-complex-keys
    ///
    /// # Examples
    ///
    /// ```
    /// use std::collections::HashMap;
    ///
    /// let mut map = HashMap::new();
    /// assert_eq!(map.insert(37, "a"), None);
    /// assert_eq!(map.is_empty(), false);
    ///
    /// map.insert(37, "b");
    /// assert_eq!(map.insert(37, "c"), Some("b"));
    /// assert_eq!(map[&37], "c");
    /// ```
    #[inline]
    pub fn insert(&mut self, k: K, v: V) -> Option<V> {
        self.base.insert(k, v)
    }
}

impl<K, V, S> Default for HashMap<K, V, S>
where
    S: Default + BuildHasher, K: Eq + Hash,
{
    /// Creates an empty `HashMap<K, V, S>`, with the `Default` value for the hasher.
    #[inline]
    fn default() -> HashMap<K, V, S> {
        HashMap::with_hasher(Default::default())
    }
}

/// `RandomState` 是 [`HashMap`] 类型的默认状态。
///
/// 特定的实例 `RandomState` 将创建 [`Hasher`] 的相同实例，但是由两个不同的 `RandomState` 实例创建的哈希对于相同的值不太可能产生相同的结果。
///
///
/// # Examples
///
/// ```
/// use std::collections::HashMap;
/// use std::collections::hash_map::RandomState;
///
/// let s = RandomState::new();
/// let mut map = HashMap::with_hasher(s);
/// map.insert(1, 2);
/// ```
///
#[derive(Clone)]
pub struct RandomState {
    k0: u64,
    k1: u64,
}

impl RandomState {
    /// 创建一个用随机键初始化的新 `RandomState`。
    ///
    /// # Examples
    ///
    /// ```
    /// use std::collections::hash_map::RandomState;
    ///
    /// let s = RandomState::new();
    /// ```
    #[inline]
    #[allow(deprecated)]
    // rand
    #[must_use]
    pub fn new() -> RandomState {
        // 从历史上看，此函数不缓存操作系统中的键，而总是简单地两次调用 `rand::thread_rng().gen()`。
        // 但是，在 #31356 中发现，由于我们定期从操作系统重新 seed，所以在线程上创建许多 hashmap 时，这可能会导致速度过慢。
        //
        // 为了解决此性能陷阱，我们按线程缓存了第一组随机生成的密钥。
        //
        // 后来在 #36481 中，发现确定性迭代顺序可以允许某种形式的 DOS 攻击。
        // 为了解决这个问题，我们在每次 RandomState 创建时增加一个种子，为每个对应的 HashMap 赋予不同的迭代顺序。
        //
        //
        //
        //
        let random = random::ax_random();
        // TODO: add random support, how to support thread safety?
        RandomState {
            k0: (random & 0xffff_ffff) as u64,
            k1: ((random >> 32) & 0xffff_ffff) as u64
        }
    }
}

impl BuildHasher for RandomState {
    type Hasher = DefaultHasher;
    #[inline]
    #[allow(deprecated)]
    fn build_hasher(&self) -> DefaultHasher {
        DefaultHasher(SipHasher13::new_with_keys(self.k0, self.k1))
    }
}

/// The default [`Hasher`] used by [`RandomState`].
///
/// The internal algorithm is not specified, and so it and its hashes should
/// not be relied upon over releases.
#[allow(deprecated)]
#[derive(Clone, Debug)]
pub struct DefaultHasher(SipHasher13);

impl DefaultHasher {
    /// Creates a new `DefaultHasher`.
    ///
    /// This hasher is not guaranteed to be the same as all other
    /// `DefaultHasher` instances, but is the same as all other `DefaultHasher`
    /// instances created through `new` or `default`.
    #[inline]
    #[allow(deprecated)]
    #[must_use]
    pub const fn new() -> DefaultHasher {
        DefaultHasher(SipHasher13::new_with_keys(0, 0))
    }
}

impl Default for DefaultHasher {
    /// Creates a new `DefaultHasher` using [`new`].
    /// See its documentation for more.
    ///
    /// [`new`]: DefaultHasher::new
    #[inline]
    fn default() -> DefaultHasher {
        DefaultHasher::new()
    }
}

impl Hasher for DefaultHasher {
    // The underlying `SipHasher13` doesn't override the other
    // `write_*` methods, so it's ok not to forward them here.

    #[inline]
    fn write(&mut self, msg: &[u8]) {
        self.0.write(msg)
    }

    #[inline]
    fn write_str(&mut self, s: &str) {
        self.0.write_str(s);
    }

    #[inline]
    fn finish(&self) -> u64 {
        self.0.finish()
    }
}

impl Default for RandomState {
    /// Constructs a new `RandomState`.
    #[inline]
    fn default() -> RandomState {
        RandomState::new()
    }
}

/// `HashMap` 条目上的迭代器。
///
/// 该 `struct` 是通过 [`HashMap`] 上的 [`iter`] 方法创建的。
/// 有关更多信息，请参见其文档。
///
/// [`iter`]: HashMap::iter
///
/// # Example
///
/// ```
/// use std::collections::HashMap;
///
/// let map = HashMap::from([
///     ("a", 1),
/// ]);
/// let iter = map.iter();
/// ```
pub struct Iter<'a, K: 'a, V: 'a> {
    base: base::Iter<'a, K, V>,
}

// FIXME(#26925) 删除以支持 `#[derive(Clone)]`
impl<K, V> Clone for Iter<'_, K, V> {
    #[inline]
    fn clone(&self) -> Self {
        Iter { base: self.base.clone() }
    }
}

impl<'a, K, V> Iterator for Iter<'a, K, V> {
    type Item = (&'a K, &'a V);

    #[inline]
    fn next(&mut self) -> Option<(&'a K, &'a V)> {
        self.base.next()
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.base.size_hint()
    }
}
