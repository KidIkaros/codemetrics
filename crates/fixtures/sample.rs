//! Sample Rust module for testing.

use std::fs;
use std::path::Path;

/// Helper doubles a value.
pub fn helper(x: i32) -> i32 {
    x * 2
}

/// Process a list of items.
pub fn process(items: &[i32]) -> i32 {
    // TODO: handle empty slice
    let mut total = 0;
    for item in items {
        total += helper(*item);
    }
    total
}
