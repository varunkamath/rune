"""
Mathematical operations module for basic arithmetic
"""

def add_numbers(a: float, b: float) -> float:
    """Add two numbers together and return the sum."""
    return a + b

def subtract_numbers(a: float, b: float) -> float:
    """Subtract second number from first and return difference."""
    return a - b

def multiply_values(x: float, y: float) -> float:
    """Multiply two values and return the product."""
    return x * y

def divide_safely(numerator: float, denominator: float) -> float:
    """Divide numerator by denominator with zero check."""
    if denominator == 0:
        raise ValueError("Cannot divide by zero")
    return numerator / denominator

def calculate_average(numbers: list) -> float:
    """Calculate the mean of a list of numbers."""
    if not numbers:
        return 0
    return sum(numbers) / len(numbers)