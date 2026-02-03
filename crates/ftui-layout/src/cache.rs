//! Layout cache for memoizing layout computation results.
//!
//! This module provides [`LayoutCache`] which caches the `Vec<Rect>` results from
//! layout computations to avoid redundant constraint solving during rendering.
//!
//! # Overview
//!
//! Layout computation (constraint solving, flex distribution) can be expensive for
//! complex nested layouts. During a single frame, the same layout may be queried
//! multiple times with identical parameters. The cache eliminates this redundancy.
//!
//! # Usage
//!
//! ```ignore
//! use ftui_layout::{Flex, Constraint, LayoutCache, LayoutCacheKey, Direction};
//! use ftui_core::geometry::Rect;
//!
//! let mut cache = LayoutCache::new(64);
//!
//! let flex = Flex::horizontal()
//!     .constraints([Constraint::Percentage(50.0), Constraint::Fill]);
//!
//! let area = Rect::new(0, 0, 80, 24);
//!
//! // First call computes and caches
//! let rects = flex.split_cached(area, &mut cache);
//!
//! // Second call returns cached result
//! let cached = flex.split_cached(area, &mut cache);
//! ```
//!
//! # Invalidation
//!
//! ## Generation-Based (Primary)
//!
//! Call [`LayoutCache::invalidate_all()`] after any state change affecting layouts:
//!
//! ```ignore
//! match msg {
//!     Msg::DataChanged(_) => {
//!         self.layout_cache.invalidate_all();
//!     }
//!     Msg::Resize(_) => {
//!         // Area is part of cache key, no invalidation needed!
//!     }
//! }
//! ```
//!
//! # Cache Eviction
//!
//! The cache uses LRU (Least Recently Used) eviction when at capacity.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};

use ftui_core::geometry::Rect;

use crate::{Constraint, Direction, LayoutSizeHint};

/// Key for layout cache lookups.
///
/// Includes all parameters that affect layout computation:
/// - The available area (stored as components for Hash)
/// - A fingerprint of all constraints
/// - The layout direction
/// - Optionally, a fingerprint of intrinsic size hints
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct LayoutCacheKey {
    /// Area x-coordinate.
    pub area_x: u16,
    /// Area y-coordinate.
    pub area_y: u16,
    /// Area width.
    pub area_width: u16,
    /// Area height.
    pub area_height: u16,
    /// Hash fingerprint of constraints.
    pub constraints_hash: u64,
    /// Layout direction.
    pub direction: Direction,
    /// Hash fingerprint of intrinsic sizes (if using FitContent).
    pub intrinsics_hash: Option<u64>,
}

impl LayoutCacheKey {
    /// Create a new cache key from layout parameters.
    ///
    /// # Arguments
    ///
    /// * `area` - The available rectangle for layout
    /// * `constraints` - The constraint list
    /// * `direction` - Horizontal or Vertical layout
    /// * `intrinsics` - Optional size hints for FitContent constraints
    pub fn new(
        area: Rect,
        constraints: &[Constraint],
        direction: Direction,
        intrinsics: Option<&[LayoutSizeHint]>,
    ) -> Self {
        Self {
            area_x: area.x,
            area_y: area.y,
            area_width: area.width,
            area_height: area.height,
            constraints_hash: Self::hash_constraints(constraints),
            direction,
            intrinsics_hash: intrinsics.map(Self::hash_intrinsics),
        }
    }

    /// Reconstruct the area Rect from cached components.
    #[inline]
    pub fn area(&self) -> Rect {
        Rect::new(self.area_x, self.area_y, self.area_width, self.area_height)
    }

    /// Hash a slice of constraints.
    fn hash_constraints(constraints: &[Constraint]) -> u64 {
        let mut hasher = DefaultHasher::new();
        for c in constraints {
            // Hash each constraint's discriminant and value
            std::mem::discriminant(c).hash(&mut hasher);
            match c {
                Constraint::Fixed(v) => v.hash(&mut hasher),
                Constraint::Percentage(p) => p.to_bits().hash(&mut hasher),
                Constraint::Min(v) => v.hash(&mut hasher),
                Constraint::Max(v) => v.hash(&mut hasher),
                Constraint::Ratio(n, d) => {
                    n.hash(&mut hasher);
                    d.hash(&mut hasher);
                }
                Constraint::Fill => {}
                Constraint::FitContent => {}
                Constraint::FitContentBounded { min, max } => {
                    min.hash(&mut hasher);
                    max.hash(&mut hasher);
                }
                Constraint::FitMin => {}
            }
        }
        hasher.finish()
    }

    /// Hash a slice of intrinsic size hints.
    fn hash_intrinsics(intrinsics: &[LayoutSizeHint]) -> u64 {
        let mut hasher = DefaultHasher::new();
        for hint in intrinsics {
            hint.min.hash(&mut hasher);
            hint.preferred.hash(&mut hasher);
            hint.max.hash(&mut hasher);
        }
        hasher.finish()
    }
}

/// Cached layout result with metadata for eviction.
#[derive(Clone, Debug)]
struct CachedLayoutEntry {
    /// The cached layout rectangles.
    chunks: Vec<Rect>,
    /// Generation when this entry was created/updated.
    generation: u64,
    /// Access count for LRU eviction.
    access_count: u32,
}

/// Statistics about layout cache performance.
#[derive(Debug, Clone, Default)]
pub struct LayoutCacheStats {
    /// Number of entries currently in the cache.
    pub entries: usize,
    /// Total cache hits since creation or last reset.
    pub hits: u64,
    /// Total cache misses since creation or last reset.
    pub misses: u64,
    /// Hit rate as a fraction (0.0 to 1.0).
    pub hit_rate: f64,
}

/// Cache for layout computation results.
///
/// Stores `Vec<Rect>` results keyed by [`LayoutCacheKey`] to avoid redundant
/// constraint solving during rendering.
///
/// # Capacity
///
/// The cache has a fixed maximum capacity. When full, the least recently used
/// entries are evicted to make room for new ones.
///
/// # Generation-Based Invalidation
///
/// Each entry is tagged with a generation number. Calling [`invalidate_all()`]
/// bumps the generation, making all existing entries stale.
///
/// [`invalidate_all()`]: LayoutCache::invalidate_all
#[derive(Debug)]
pub struct LayoutCache {
    entries: HashMap<LayoutCacheKey, CachedLayoutEntry>,
    generation: u64,
    max_entries: usize,
    hits: u64,
    misses: u64,
}

impl LayoutCache {
    /// Create a new cache with the specified maximum capacity.
    ///
    /// # Arguments
    ///
    /// * `max_entries` - Maximum number of entries before LRU eviction occurs.
    ///   A typical value is 64-256 for most UIs.
    ///
    /// # Example
    ///
    /// ```
    /// use ftui_layout::LayoutCache;
    /// let cache = LayoutCache::new(64);
    /// ```
    #[inline]
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: HashMap::with_capacity(max_entries),
            generation: 0,
            max_entries,
            hits: 0,
            misses: 0,
        }
    }

    /// Get cached layout or compute and cache a new one.
    ///
    /// If a valid (same generation) cache entry exists for the given key,
    /// returns a clone of it. Otherwise, calls the `compute` closure,
    /// caches the result, and returns it.
    ///
    /// # Arguments
    ///
    /// * `key` - The cache key identifying this layout computation
    /// * `compute` - Closure to compute the layout if not cached
    ///
    /// # Example
    ///
    /// ```ignore
    /// let key = LayoutCacheKey::new(area, &constraints, Direction::Horizontal, None);
    /// let rects = cache.get_or_compute(key, || flex.split(area));
    /// ```
    pub fn get_or_compute<F>(&mut self, key: LayoutCacheKey, compute: F) -> Vec<Rect>
    where
        F: FnOnce() -> Vec<Rect>,
    {
        // Check for existing valid entry
        if let Some(entry) = self.entries.get_mut(&key)
            && entry.generation == self.generation
        {
            self.hits += 1;
            entry.access_count = entry.access_count.saturating_add(1);
            return entry.chunks.clone();
        }

        // Cache miss - compute the value
        self.misses += 1;
        let chunks = compute();

        // Evict if at capacity
        if self.entries.len() >= self.max_entries {
            self.evict_lru();
        }

        // Insert new entry
        self.entries.insert(
            key,
            CachedLayoutEntry {
                chunks: chunks.clone(),
                generation: self.generation,
                access_count: 1,
            },
        );

        chunks
    }

    /// Invalidate all entries by bumping the generation.
    ///
    /// Existing entries become stale and will be recomputed on next access.
    /// This is an O(1) operation - entries are not immediately removed.
    ///
    /// # When to Call
    ///
    /// Call this after any state change that affects layout:
    /// - Model data changes that affect widget content
    /// - Theme/font changes that affect sizing
    ///
    /// # Note
    ///
    /// Resize events don't require invalidation because the area
    /// is part of the cache key.
    #[inline]
    pub fn invalidate_all(&mut self) {
        self.generation = self.generation.wrapping_add(1);
    }

    /// Get current cache statistics.
    ///
    /// Returns hit/miss counts and the current hit rate.
    pub fn stats(&self) -> LayoutCacheStats {
        let total = self.hits + self.misses;
        LayoutCacheStats {
            entries: self.entries.len(),
            hits: self.hits,
            misses: self.misses,
            hit_rate: if total > 0 {
                self.hits as f64 / total as f64
            } else {
                0.0
            },
        }
    }

    /// Reset statistics counters to zero.
    ///
    /// Useful for measuring hit rate over a specific period (e.g., per frame).
    #[inline]
    pub fn reset_stats(&mut self) {
        self.hits = 0;
        self.misses = 0;
    }

    /// Clear all entries from the cache.
    ///
    /// Unlike [`invalidate_all()`], this immediately frees memory.
    ///
    /// [`invalidate_all()`]: LayoutCache::invalidate_all
    #[inline]
    pub fn clear(&mut self) {
        self.entries.clear();
        self.generation = self.generation.wrapping_add(1);
    }

    /// Returns the current number of entries in the cache.
    #[inline]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if the cache is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the maximum capacity of the cache.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.max_entries
    }

    /// Evict the least recently used entry.
    fn evict_lru(&mut self) {
        if let Some(key) = self
            .entries
            .iter()
            .min_by_key(|(_, e)| e.access_count)
            .map(|(k, _)| *k)
        {
            self.entries.remove(&key);
        }
    }
}

impl Default for LayoutCache {
    /// Creates a cache with default capacity of 64 entries.
    fn default() -> Self {
        Self::new(64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_key(width: u16, height: u16) -> LayoutCacheKey {
        LayoutCacheKey::new(
            Rect::new(0, 0, width, height),
            &[Constraint::Percentage(50.0), Constraint::Fill],
            Direction::Horizontal,
            None,
        )
    }

    // --- LayoutCacheKey tests ---

    #[test]
    fn same_params_produce_same_key() {
        let k1 = make_key(80, 24);
        let k2 = make_key(80, 24);
        assert_eq!(k1, k2);
    }

    #[test]
    fn different_area_different_key() {
        let k1 = make_key(80, 24);
        let k2 = make_key(120, 40);
        assert_ne!(k1, k2);
    }

    #[test]
    fn different_constraints_different_key() {
        let k1 = LayoutCacheKey::new(
            Rect::new(0, 0, 80, 24),
            &[Constraint::Fixed(20)],
            Direction::Horizontal,
            None,
        );
        let k2 = LayoutCacheKey::new(
            Rect::new(0, 0, 80, 24),
            &[Constraint::Fixed(30)],
            Direction::Horizontal,
            None,
        );
        assert_ne!(k1, k2);
    }

    #[test]
    fn different_direction_different_key() {
        let k1 = LayoutCacheKey::new(
            Rect::new(0, 0, 80, 24),
            &[Constraint::Fill],
            Direction::Horizontal,
            None,
        );
        let k2 = LayoutCacheKey::new(
            Rect::new(0, 0, 80, 24),
            &[Constraint::Fill],
            Direction::Vertical,
            None,
        );
        assert_ne!(k1, k2);
    }

    #[test]
    fn different_intrinsics_different_key() {
        let hints1 = [LayoutSizeHint {
            min: 10,
            preferred: 20,
            max: None,
        }];
        let hints2 = [LayoutSizeHint {
            min: 10,
            preferred: 30,
            max: None,
        }];

        let k1 = LayoutCacheKey::new(
            Rect::new(0, 0, 80, 24),
            &[Constraint::FitContent],
            Direction::Horizontal,
            Some(&hints1),
        );
        let k2 = LayoutCacheKey::new(
            Rect::new(0, 0, 80, 24),
            &[Constraint::FitContent],
            Direction::Horizontal,
            Some(&hints2),
        );
        assert_ne!(k1, k2);
    }

    // --- LayoutCache tests ---

    #[test]
    fn cache_returns_same_result() {
        let mut cache = LayoutCache::new(100);
        let key = make_key(80, 24);

        let mut compute_count = 0;
        let compute = || {
            compute_count += 1;
            vec![Rect::new(0, 0, 40, 24), Rect::new(40, 0, 40, 24)]
        };

        let r1 = cache.get_or_compute(key, compute);
        let r2 = cache.get_or_compute(key, || panic!("should not call"));

        assert_eq!(r1, r2);
        assert_eq!(compute_count, 1);
    }

    #[test]
    fn different_area_is_cache_miss() {
        let mut cache = LayoutCache::new(100);

        let mut compute_count = 0;
        let mut compute = || {
            compute_count += 1;
            vec![Rect::default()]
        };

        let k1 = make_key(80, 24);
        let k2 = make_key(120, 40);

        cache.get_or_compute(k1, &mut compute);
        cache.get_or_compute(k2, &mut compute);

        assert_eq!(compute_count, 2);
    }

    #[test]
    fn invalidation_clears_cache() {
        let mut cache = LayoutCache::new(100);
        let key = make_key(80, 24);

        let mut compute_count = 0;
        let mut compute = || {
            compute_count += 1;
            vec![]
        };

        cache.get_or_compute(key, &mut compute);
        cache.invalidate_all();
        cache.get_or_compute(key, &mut compute);

        assert_eq!(compute_count, 2);
    }

    #[test]
    fn lru_eviction_works() {
        let mut cache = LayoutCache::new(2);

        let k1 = make_key(10, 10);
        let k2 = make_key(20, 20);
        let k3 = make_key(30, 30);

        // Insert two entries
        cache.get_or_compute(k1, || vec![Rect::new(0, 0, 10, 10)]);
        cache.get_or_compute(k2, || vec![Rect::new(0, 0, 20, 20)]);

        // Access k1 again (increases access count)
        cache.get_or_compute(k1, || panic!("k1 should hit"));

        // Insert k3, should evict k2 (least accessed)
        cache.get_or_compute(k3, || vec![Rect::new(0, 0, 30, 30)]);

        assert_eq!(cache.len(), 2);

        // k2 should be evicted
        let mut was_called = false;
        cache.get_or_compute(k2, || {
            was_called = true;
            vec![]
        });
        assert!(was_called, "k2 should have been evicted");

        // k1 should still be cached
        cache.get_or_compute(k1, || panic!("k1 should still be cached"));
    }

    #[test]
    fn stats_track_hits_and_misses() {
        let mut cache = LayoutCache::new(100);

        let k1 = make_key(80, 24);
        let k2 = make_key(120, 40);

        cache.get_or_compute(k1, Vec::new); // miss
        cache.get_or_compute(k1, || panic!("hit")); // hit
        cache.get_or_compute(k2, Vec::new); // miss

        let stats = cache.stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 2);
        assert!((stats.hit_rate - 1.0 / 3.0).abs() < 0.01);
    }

    #[test]
    fn reset_stats_clears_counters() {
        let mut cache = LayoutCache::new(100);
        let key = make_key(80, 24);

        cache.get_or_compute(key, Vec::new);
        cache.get_or_compute(key, || panic!("hit"));

        let stats = cache.stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);

        cache.reset_stats();

        let stats = cache.stats();
        assert_eq!(stats.hits, 0);
        assert_eq!(stats.misses, 0);
        assert_eq!(stats.hit_rate, 0.0);
    }

    #[test]
    fn clear_removes_all_entries() {
        let mut cache = LayoutCache::new(100);

        cache.get_or_compute(make_key(80, 24), Vec::new);
        cache.get_or_compute(make_key(120, 40), Vec::new);

        assert_eq!(cache.len(), 2);

        cache.clear();

        assert_eq!(cache.len(), 0);
        assert!(cache.is_empty());

        // All entries should miss now
        let mut was_called = false;
        cache.get_or_compute(make_key(80, 24), || {
            was_called = true;
            vec![]
        });
        assert!(was_called);
    }

    #[test]
    fn default_capacity_is_64() {
        let cache = LayoutCache::default();
        assert_eq!(cache.capacity(), 64);
    }

    #[test]
    fn generation_wraps_around() {
        let mut cache = LayoutCache::new(100);
        cache.generation = u64::MAX;
        cache.invalidate_all();
        assert_eq!(cache.generation, 0);
    }

    // --- Constraint hashing tests ---

    #[test]
    fn constraint_hash_is_stable() {
        let constraints = [
            Constraint::Fixed(20),
            Constraint::Percentage(50.0),
            Constraint::Min(10),
        ];

        let h1 = LayoutCacheKey::hash_constraints(&constraints);
        let h2 = LayoutCacheKey::hash_constraints(&constraints);

        assert_eq!(h1, h2);
    }

    #[test]
    fn different_constraint_values_different_hash() {
        let c1 = [Constraint::Fixed(20)];
        let c2 = [Constraint::Fixed(30)];

        let h1 = LayoutCacheKey::hash_constraints(&c1);
        let h2 = LayoutCacheKey::hash_constraints(&c2);

        assert_ne!(h1, h2);
    }

    #[test]
    fn different_constraint_types_different_hash() {
        let c1 = [Constraint::Fixed(20)];
        let c2 = [Constraint::Min(20)];

        let h1 = LayoutCacheKey::hash_constraints(&c1);
        let h2 = LayoutCacheKey::hash_constraints(&c2);

        assert_ne!(h1, h2);
    }

    #[test]
    fn fit_content_bounded_values_in_hash() {
        let c1 = [Constraint::FitContentBounded { min: 10, max: 50 }];
        let c2 = [Constraint::FitContentBounded { min: 10, max: 60 }];

        let h1 = LayoutCacheKey::hash_constraints(&c1);
        let h2 = LayoutCacheKey::hash_constraints(&c2);

        assert_ne!(h1, h2);
    }

    // --- Intrinsics hashing tests ---

    #[test]
    fn intrinsics_hash_is_stable() {
        let hints = [
            LayoutSizeHint {
                min: 10,
                preferred: 20,
                max: Some(30),
            },
            LayoutSizeHint {
                min: 5,
                preferred: 15,
                max: None,
            },
        ];

        let h1 = LayoutCacheKey::hash_intrinsics(&hints);
        let h2 = LayoutCacheKey::hash_intrinsics(&hints);

        assert_eq!(h1, h2);
    }

    #[test]
    fn different_intrinsics_different_hash() {
        let h1 = [LayoutSizeHint {
            min: 10,
            preferred: 20,
            max: None,
        }];
        let h2 = [LayoutSizeHint {
            min: 10,
            preferred: 25,
            max: None,
        }];

        let hash1 = LayoutCacheKey::hash_intrinsics(&h1);
        let hash2 = LayoutCacheKey::hash_intrinsics(&h2);

        assert_ne!(hash1, hash2);
    }

    // --- Property-like tests ---

    #[test]
    fn cache_is_deterministic() {
        let mut cache1 = LayoutCache::new(100);
        let mut cache2 = LayoutCache::new(100);

        for i in 0..10u16 {
            let key = make_key(i * 10, i * 5);
            let result = vec![Rect::new(0, 0, i, i)];

            cache1.get_or_compute(key, || result.clone());
            cache2.get_or_compute(key, || result);
        }

        assert_eq!(cache1.stats().entries, cache2.stats().entries);
        assert_eq!(cache1.stats().misses, cache2.stats().misses);
    }

    #[test]
    fn hit_count_increments_on_each_access() {
        let mut cache = LayoutCache::new(100);
        let key = make_key(80, 24);

        // First access is a miss
        cache.get_or_compute(key, Vec::new);

        // Subsequent accesses are hits
        for _ in 0..5 {
            cache.get_or_compute(key, || panic!("should hit"));
        }

        let stats = cache.stats();
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.hits, 5);
    }
}
