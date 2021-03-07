use std::time::Duration;
use crate::resources::Time;
use crate::consts::DURATION_ZERO;
use specs::Read;

pub struct Timer {
    elapsed: Duration,
    duration: Duration,
}

impl Timer {
    pub fn tick(&mut self, dt: &Read<Time>) -> bool {
        self.elapsed += dt.delta;
        self.elapsed > self.duration
    }

    pub fn new(duration: Duration) -> Self {
        Self {
            elapsed: DURATION_ZERO,
            duration,
        }
    }

    pub fn reset(&mut self) {
        self.elapsed = DURATION_ZERO;
    }
}

pub fn min_f32(f1: f32, f2: f32) -> f32 {
    if f1 < f2 {
        f1
    } else {
        f2
    }
}

pub fn min_f64(f1: f64, f2: f64) -> f64 {
    if f1 < f2 {
        f1
    } else {
        f2
    }
}

pub fn max_f64(f1: f64, f2: f64) -> f64 {
    if f1 > f2 {
        f1
    } else {
        f2
    }
}

pub fn circle_contains_point(
    point_x: f32,
    point_y: f32,
    circle_x: f32,
    circle_y: f32,
    circle_radius: f32,
) -> bool {
    ((point_x - circle_x).powf(2.0) + (point_y - circle_y).powf(2.0)).sqrt() < circle_radius
}

pub fn is_bit_set(
    src: u8,
    bit_mask: u8,
) -> bool {
    src & bit_mask == bit_mask
}

pub fn is_nth_bit_set(
    src: u8,
    bit_index: u8,
) -> bool {
    is_bit_set(src, 1 << bit_index)
}

#[test]
fn bit_arithmetic_test() {
    assert!(is_nth_bit_set(0b_0000_0001, 0));
    assert!(!is_nth_bit_set(0b_0000_0010, 0));
    assert!(is_nth_bit_set(0b_0000_0011, 1));
    assert!(is_nth_bit_set(0b_0000_0101, 2));
    assert!(is_nth_bit_set(0b_1000_0101, 7));
}

pub fn lerp(
    value_1: f32,
    value_2: f32,
    t: f32,
) -> f32 {
    return (1.0 - t) * value_1 + t * value_2;
}

#[test]
fn lerp_test() {
    assert_eq!(lerp(0.0, 50.0, 0.5), 25.0);
}


pub fn btree_less<K, V>(btree: &std::collections::BTreeMap<K, V>, key: K) -> Option<&V>
    where K: Ord {
    btree.range(..key).next_back().map(|it| it.1)
}

pub fn btree_gt<K, V>(btree: &std::collections::BTreeMap<K, V>, key: K) -> Option<&V>
    where K: Ord {
    btree.range(key..).next().map(|it| it.1)
}

#[test]
fn btree_utils_test() {
    let mut map = std::collections::BTreeMap::new();
    map.insert(1, 1);
    map.insert(2, 2);
    map.insert(4, 4);
    map.insert(10, 10);
    assert_eq!(btree_gt(&map, 2), Some(&2));
    assert_eq!(btree_gt(&map, 3), Some(&4));
    assert_eq!(btree_less(&map, 100), Some(&10));
    assert_eq!(btree_gt(&map, 40), None);
}