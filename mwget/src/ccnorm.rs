use std::collections::HashMap;

use super::equivset::make_hashmap;

lazy_static::lazy_static! {
    static ref MAP: HashMap<char, char> = make_hashmap();
}

pub fn ccnorm(s: &str) -> String {
    s.chars().map(|c| MAP.get(&c).copied().unwrap_or(c)).collect()
}