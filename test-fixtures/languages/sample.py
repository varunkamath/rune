"""Test fixture for Python code"""
from typing import Dict, List, Optional, Callable
import math


class DataProcessor:
    """A class for processing various types of data."""
    
    def __init__(self, name: str = "default"):
        """Initialize the data processor.
        
        Args:
            name: The name of the processor
        """
        self.name = name
        self.data: List[float] = []
        self.filters: Dict[str, Callable] = {}
    
    def add_data(self, value: float) -> None:
        """Add a single data point.
        
        Args:
            value: The value to add
        """
        self.data.append(value)
    
    def calculate_mean(self) -> Optional[float]:
        """Calculate the mean of the data.
        
        Returns:
            The mean value or None if no data
        """
        if not self.data:
            return None
        return sum(self.data) / len(self.data)
    
    def calculate_median(self) -> Optional[float]:
        """Calculate the median of the data.
        
        Returns:
            The median value or None if no data
        """
        if not self.data:
            return None
        sorted_data = sorted(self.data)
        n = len(sorted_data)
        if n % 2 == 0:
            return (sorted_data[n//2 - 1] + sorted_data[n//2]) / 2
        return sorted_data[n//2]
    
    def apply_filter(self, filter_name: str) -> List[float]:
        """Apply a named filter to the data.
        
        Args:
            filter_name: The name of the filter to apply
            
        Returns:
            The filtered data
        """
        if filter_name not in self.filters:
            return self.data
        return [self.filters[filter_name](x) for x in self.data]
    
    @staticmethod
    def standard_deviation(data: List[float]) -> float:
        """Calculate standard deviation.
        
        Args:
            data: The data points
            
        Returns:
            The standard deviation
        """
        if len(data) < 2:
            return 0.0
        mean = sum(data) / len(data)
        variance = sum((x - mean) ** 2 for x in data) / (len(data) - 1)
        return math.sqrt(variance)
    
    @classmethod
    def from_csv(cls, filepath: str) -> 'DataProcessor':
        """Create a DataProcessor from a CSV file.
        
        Args:
            filepath: Path to the CSV file
            
        Returns:
            A new DataProcessor instance
        """
        processor = cls(name=f"csv_{filepath}")
        # Implementation would read CSV here
        return processor


def fibonacci(n: int) -> int:
    """Calculate the nth Fibonacci number.
    
    Args:
        n: The position in the Fibonacci sequence
        
    Returns:
        The nth Fibonacci number
    """
    if n <= 0:
        return 0
    elif n == 1:
        return 1
    else:
        return fibonacci(n - 1) + fibonacci(n - 2)


async def async_process_data(data: List[float]) -> Dict[str, float]:
    """Asynchronously process data and return statistics.
    
    Args:
        data: The data to process
        
    Returns:
        A dictionary of statistics
    """
    # Simulate async processing
    import asyncio
    await asyncio.sleep(0.1)
    
    return {
        'min': min(data) if data else 0,
        'max': max(data) if data else 0,
        'sum': sum(data),
        'count': len(data),
    }


# Lambda functions for testing
square = lambda x: x ** 2
cube = lambda x: x ** 3
is_even = lambda x: x % 2 == 0