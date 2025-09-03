"""
Machine learning utilities for data preprocessing and model training
"""

import numpy as np
from typing import Tuple, List, Optional
from dataclasses import dataclass

@dataclass
class DataSplit:
    """Represents train/validation/test data splits"""
    X_train: np.ndarray
    X_val: np.ndarray
    X_test: np.ndarray
    y_train: np.ndarray
    y_val: np.ndarray
    y_test: np.ndarray

class DataPreprocessor:
    """Preprocessing pipeline for machine learning data"""

    def __init__(self):
        self.mean = None
        self.std = None
        self.feature_names = []

    def normalize_features(self, X: np.ndarray, fit: bool = True) -> np.ndarray:
        """Normalize features using z-score normalization"""
        if fit:
            self.mean = np.mean(X, axis=0)
            self.std = np.std(X, axis=0)
            self.std[self.std == 0] = 1  # Avoid division by zero

        if self.mean is None or self.std is None:
            raise ValueError("Preprocessor not fitted. Call with fit=True first.")

        return (X - self.mean) / self.std

    def train_test_split(self, X: np.ndarray, y: np.ndarray,
                        test_size: float = 0.2,
                        val_size: float = 0.1,
                        random_state: int = 42) -> DataSplit:
        """Split data into training, validation, and test sets"""
        np.random.seed(random_state)
        n_samples = X.shape[0]
        indices = np.random.permutation(n_samples)

        test_split = int(n_samples * test_size)
        val_split = int(n_samples * val_size)

        test_indices = indices[:test_split]
        val_indices = indices[test_split:test_split + val_split]
        train_indices = indices[test_split + val_split:]

        return DataSplit(
            X_train=X[train_indices],
            X_val=X[val_indices],
            X_test=X[test_indices],
            y_train=y[train_indices],
            y_val=y[val_indices],
            y_test=y[test_indices]
        )

    def remove_outliers(self, X: np.ndarray, threshold: float = 3.0) -> Tuple[np.ndarray, np.ndarray]:
        """Remove outliers using z-score method"""
        z_scores = np.abs((X - np.mean(X, axis=0)) / np.std(X, axis=0))
        outlier_mask = np.all(z_scores < threshold, axis=1)
        return X[outlier_mask], outlier_mask

class NeuralNetwork:
    """Simple neural network implementation for classification"""

    def __init__(self, layer_sizes: List[int], learning_rate: float = 0.01):
        self.layer_sizes = layer_sizes
        self.learning_rate = learning_rate
        self.weights = []
        self.biases = []
        self._initialize_parameters()

    def _initialize_parameters(self):
        """Initialize weights and biases using Xavier initialization"""
        for i in range(len(self.layer_sizes) - 1):
            weight = np.random.randn(self.layer_sizes[i], self.layer_sizes[i+1]) * np.sqrt(2.0 / self.layer_sizes[i])
            bias = np.zeros((1, self.layer_sizes[i+1]))
            self.weights.append(weight)
            self.biases.append(bias)

    def forward_propagation(self, X: np.ndarray) -> List[np.ndarray]:
        """Perform forward pass through the network"""
        activations = [X]
        current = X

        for i, (W, b) in enumerate(zip(self.weights, self.biases)):
            z = np.dot(current, W) + b
            if i < len(self.weights) - 1:
                # ReLU activation for hidden layers
                current = np.maximum(0, z)
            else:
                # Softmax for output layer
                exp_z = np.exp(z - np.max(z, axis=1, keepdims=True))
                current = exp_z / np.sum(exp_z, axis=1, keepdims=True)
            activations.append(current)

        return activations

    def backward_propagation(self, X: np.ndarray, y: np.ndarray,
                           activations: List[np.ndarray]) -> Tuple[List[np.ndarray], List[np.ndarray]]:
        """Compute gradients using backpropagation"""
        m = X.shape[0]
        gradients_W = []
        gradients_b = []

        # Output layer gradient
        dz = activations[-1] - y

        for i in range(len(self.weights) - 1, -1, -1):
            dW = np.dot(activations[i].T, dz) / m
            db = np.sum(dz, axis=0, keepdims=True) / m

            gradients_W.insert(0, dW)
            gradients_b.insert(0, db)

            if i > 0:
                # Backpropagate through ReLU
                dz = np.dot(dz, self.weights[i].T)
                dz[activations[i] <= 0] = 0

        return gradients_W, gradients_b

    def train_epoch(self, X: np.ndarray, y: np.ndarray) -> float:
        """Train the network for one epoch"""
        # Forward pass
        activations = self.forward_propagation(X)

        # Compute loss (cross-entropy)
        predictions = activations[-1]
        loss = -np.mean(np.sum(y * np.log(predictions + 1e-8), axis=1))

        # Backward pass
        gradients_W, gradients_b = self.backward_propagation(X, y, activations)

        # Update parameters
        for i in range(len(self.weights)):
            self.weights[i] -= self.learning_rate * gradients_W[i]
            self.biases[i] -= self.learning_rate * gradients_b[i]

        return loss

    def predict(self, X: np.ndarray) -> np.ndarray:
        """Make predictions on input data"""
        activations = self.forward_propagation(X)
        return np.argmax(activations[-1], axis=1)

def calculate_accuracy(y_true: np.ndarray, y_pred: np.ndarray) -> float:
    """Calculate classification accuracy"""
    return np.mean(y_true == y_pred)

def confusion_matrix(y_true: np.ndarray, y_pred: np.ndarray, n_classes: int) -> np.ndarray:
    """Generate confusion matrix for classification results"""
    matrix = np.zeros((n_classes, n_classes), dtype=int)
    for true, pred in zip(y_true, y_pred):
        matrix[true, pred] += 1
    return matrix
