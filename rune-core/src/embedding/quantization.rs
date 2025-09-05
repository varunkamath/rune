use serde::{Deserialize, Serialize};
use tracing::{info, warn};

/// Quantization mode for vector storage
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuantizationMode {
    /// No quantization - full float32 precision (384 dims * 4 bytes = 1536 bytes per vector)
    None,
    /// Scalar quantization to int8 (384 dims * 1 byte = 384 bytes per vector, 75% reduction)
    Scalar,
    /// Binary quantization to 1-bit (384 dims / 8 = 48 bytes per vector, 97% reduction)
    Binary,
    /// Asymmetric: Binary storage + Scalar queries (best accuracy with low memory)
    Asymmetric,
}

impl Default for QuantizationMode {
    fn default() -> Self {
        // Default to scalar for good balance of accuracy and memory
        Self::Scalar
    }
}

impl QuantizationMode {
    /// Parse from environment variable
    pub fn from_env() -> Self {
        match std::env::var("RUNE_QUANTIZATION_MODE")
            .unwrap_or_else(|_| "scalar".to_string())
            .to_lowercase()
            .as_str()
        {
            "none" => Self::None,
            "scalar" => Self::Scalar,
            "binary" => Self::Binary,
            "asymmetric" => Self::Asymmetric,
            _ => {
                warn!("Invalid RUNE_QUANTIZATION_MODE, defaulting to scalar");
                Self::default()
            },
        }
    }

    /// Get memory reduction percentage
    pub fn memory_reduction(&self) -> f32 {
        match self {
            Self::None => 0.0,
            Self::Scalar => 75.0,
            Self::Binary | Self::Asymmetric => 97.0,
        }
    }

    /// Get bytes per vector for 384-dimensional embeddings
    pub fn bytes_per_vector(&self) -> usize {
        match self {
            Self::None => 384 * 4,                 // float32
            Self::Scalar => 384,                   // int8
            Self::Binary | Self::Asymmetric => 48, // 384 bits / 8
        }
    }
}

/// Configuration for quantization with Qdrant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantizationConfig {
    /// Quantization mode
    pub mode: QuantizationMode,

    /// Whether to always keep quantized vectors in RAM (faster but uses more memory)
    pub always_ram: bool,

    /// Oversampling factor for better accuracy (e.g., 2.0 = retrieve 2x vectors then rerank)
    pub oversampling: f32,

    /// Whether to rescore using full precision vectors (asymmetric mode)
    pub rescore: bool,
}

impl Default for QuantizationConfig {
    fn default() -> Self {
        Self {
            mode: QuantizationMode::from_env(),
            always_ram: true,  // Keep in RAM for speed
            oversampling: 2.0, // Retrieve 2x vectors for better accuracy
            rescore: true,     // Rescore with full precision when possible
        }
    }
}

impl QuantizationConfig {
    pub fn new(mode: QuantizationMode) -> Self {
        let mut config = Self {
            mode,
            ..Default::default()
        };

        // Adjust settings based on mode
        match mode {
            QuantizationMode::None => {
                config.oversampling = 1.0; // No need for oversampling
                config.rescore = false;
            },
            QuantizationMode::Binary => {
                config.oversampling = 3.0; // Higher oversampling for binary
            },
            QuantizationMode::Asymmetric => {
                config.oversampling = 2.5;
                config.rescore = true; // Always rescore in asymmetric mode
            },
            _ => {},
        }

        config
    }

    /// Log configuration details
    pub fn log_config(&self) {
        info!(
            "Quantization: mode={:?}, memory_reduction={}%, bytes_per_vector={}, oversampling={:.1}x",
            self.mode,
            self.mode.memory_reduction(),
            self.mode.bytes_per_vector(),
            self.oversampling
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quantization_mode_parsing() {
        unsafe {
            std::env::set_var("RUNE_QUANTIZATION_MODE", "binary");
        }
        assert_eq!(QuantizationMode::from_env(), QuantizationMode::Binary);

        unsafe {
            std::env::set_var("RUNE_QUANTIZATION_MODE", "SCALAR");
        }
        assert_eq!(QuantizationMode::from_env(), QuantizationMode::Scalar);

        unsafe {
            std::env::set_var("RUNE_QUANTIZATION_MODE", "invalid");
        }
        assert_eq!(QuantizationMode::from_env(), QuantizationMode::Scalar); // default

        unsafe {
            std::env::remove_var("RUNE_QUANTIZATION_MODE");
        }
    }

    #[test]
    fn test_memory_calculations() {
        assert_eq!(QuantizationMode::None.bytes_per_vector(), 1536);
        assert_eq!(QuantizationMode::Scalar.bytes_per_vector(), 384);
        assert_eq!(QuantizationMode::Binary.bytes_per_vector(), 48);

        assert_eq!(QuantizationMode::None.memory_reduction(), 0.0);
        assert_eq!(QuantizationMode::Scalar.memory_reduction(), 75.0);
        assert_eq!(QuantizationMode::Binary.memory_reduction(), 97.0);
    }

    #[test]
    fn test_config_defaults() {
        let config = QuantizationConfig::new(QuantizationMode::Binary);
        assert_eq!(config.mode, QuantizationMode::Binary);
        assert_eq!(config.oversampling, 3.0);
        assert!(config.always_ram);
        assert!(config.rescore);
    }
}
