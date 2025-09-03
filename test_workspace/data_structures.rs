// Data structure implementations in Rust

use std::collections::HashMap;

/// A simple stack implementation
pub struct Stack<T> {
    items: Vec<T>,
}

impl<T> Stack<T> {
    /// Create a new empty stack
    pub fn new() -> Self {
        Stack { items: Vec::new() }
    }

    /// Push item onto the stack
    pub fn push(&mut self, item: T) {
        self.items.push(item);
    }

    /// Pop item from the stack
    pub fn pop(&mut self) -> Option<T> {
        self.items.pop()
    }

    /// Check if stack is empty
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

/// Binary tree node
pub struct TreeNode<T> {
    value: T,
    left: Option<Box<TreeNode<T>>>,
    right: Option<Box<TreeNode<T>>>,
}

/// Calculate sum of array elements
pub fn array_sum(numbers: &[i32]) -> i32 {
    numbers.iter().sum()
}

/// Find maximum value in array
pub fn find_maximum(values: &[i32]) -> Option<i32> {
    values.iter().max().copied()
}
