// Test fixture for Rust code
use std::collections::HashMap;

/// A simple calculator struct
#[derive(Debug, Clone)]
pub struct Calculator {
    memory: f64,
    operations: HashMap<String, Box<dyn Fn(f64, f64) -> f64>>,
}

impl Calculator {
    /// Creates a new calculator instance
    pub fn new() -> Self {
        Self {
            memory: 0.0,
            operations: HashMap::new(),
        }
    }

    /// Adds two numbers
    pub fn add(&self, a: f64, b: f64) -> f64 {
        a + b
    }

    /// Subtracts b from a
    pub fn subtract(&self, a: f64, b: f64) -> f64 {
        a - b
    }

    /// Multiplies two numbers
    pub fn multiply(&self, a: f64, b: f64) -> f64 {
        a * b
    }

    /// Divides a by b
    pub fn divide(&self, a: f64, b: f64) -> Result<f64, String> {
        if b == 0.0 {
            Err("Division by zero".to_string())
        } else {
            Ok(a / b)
        }
    }

    /// Stores a value in memory
    pub fn store(&mut self, value: f64) {
        self.memory = value;
    }

    /// Recalls the stored value
    pub fn recall(&self) -> f64 {
        self.memory
    }
}

trait MathOperation {
    fn execute(&self, a: f64, b: f64) -> f64;
}

struct AddOperation;

impl MathOperation for AddOperation {
    fn execute(&self, a: f64, b: f64) -> f64 {
        a + b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_addition() {
        let calc = Calculator::new();
        assert_eq!(calc.add(2.0, 3.0), 5.0);
    }

    #[test]
    fn test_division() {
        let calc = Calculator::new();
        assert!(calc.divide(10.0, 2.0).is_ok());
        assert!(calc.divide(10.0, 0.0).is_err());
    }
}
