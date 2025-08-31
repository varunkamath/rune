use anyhow::{Context, Result};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

#[cfg(feature = "semantic")]
use qdrant_client::{
    Qdrant,
    qdrant::{
        CreateCollectionBuilder, Distance, Filter, PointStruct, SearchParamsBuilder,
        SearchPointsBuilder, UpsertPointsBuilder, VectorParamsBuilder,
    },
};

use crate::Config;

/// Manages Qdrant vector database operations
pub struct QdrantManager {
    config: Arc<Config>,
    #[cfg(feature = "semantic")]
    client: Option<Qdrant>,
    collection_name: String,
}

impl QdrantManager {
    pub async fn new(config: Arc<Config>) -> Result<Self> {
        // Generate collection name based on workspace path hash
        let workspace_hash = blake3::hash(config.workspace_dir.as_bytes())
            .to_hex()
            .chars()
            .take(16)
            .collect::<String>();
        let collection_name = format!("rune_{}", workspace_hash);

        #[cfg(feature = "semantic")]
        {
            let enable_semantic = std::env::var("RUNE_ENABLE_SEMANTIC")
                .unwrap_or_else(|_| "true".to_string())
                .parse::<bool>()
                .unwrap_or(true);

            if !enable_semantic {
                info!("Semantic search disabled by configuration");
                return Ok(Self {
                    config,
                    client: None,
                    collection_name,
                });
            }

            let qdrant_url =
                std::env::var("QDRANT_URL").unwrap_or_else(|_| "http://localhost:6333".to_string());

            info!("Connecting to Qdrant at {}...", qdrant_url);

            match Qdrant::from_url(&qdrant_url).build() {
                Ok(client) => {
                    // Check health
                    match client.health_check().await {
                        Ok(_) => {
                            info!("Successfully connected to Qdrant");

                            // Initialize collection
                            if let Err(e) = Self::init_collection(&client, &collection_name).await {
                                error!("Failed to initialize collection: {}", e);
                                return Ok(Self {
                                    config,
                                    client: None,
                                    collection_name,
                                });
                            }

                            Ok(Self {
                                config,
                                client: Some(client),
                                collection_name,
                            })
                        },
                        Err(e) => {
                            warn!(
                                "Qdrant health check failed: {}. Semantic search will be disabled.",
                                e
                            );
                            Ok(Self {
                                config,
                                client: None,
                                collection_name,
                            })
                        },
                    }
                },
                Err(e) => {
                    warn!(
                        "Failed to connect to Qdrant: {}. Semantic search will be disabled.",
                        e
                    );
                    Ok(Self {
                        config,
                        client: None,
                        collection_name,
                    })
                },
            }
        }

        #[cfg(not(feature = "semantic"))]
        {
            debug!("Semantic feature not enabled at compile time");
            Ok(Self {
                config,
                collection_name,
            })
        }
    }

    #[cfg(feature = "semantic")]
    async fn init_collection(client: &Qdrant, collection_name: &str) -> Result<()> {
        // Check if collection exists
        let collections = client.list_collections().await?;
        let exists = collections
            .collections
            .iter()
            .any(|c| c.name == collection_name);

        if !exists {
            info!("Creating collection '{}'", collection_name);

            client
                .create_collection(
                    CreateCollectionBuilder::new(collection_name)
                        .vectors_config(VectorParamsBuilder::new(256, Distance::Cosine)),
                )
                .await
                .context("Failed to create collection")?;

            info!("Collection created successfully");
        } else {
            debug!("Collection '{}' already exists", collection_name);
        }

        Ok(())
    }

    /// Store embeddings with metadata
    pub async fn store_embeddings(&self, chunks: Vec<EmbeddedChunk>) -> Result<()> {
        #[cfg(feature = "semantic")]
        {
            if let Some(ref client) = self.client {
                if chunks.is_empty() {
                    return Ok(());
                }

                debug!("Storing {} embeddings in Qdrant", chunks.len());

                let points: Vec<PointStruct> = chunks
                    .into_iter()
                    .map(|chunk| {
                        let mut payload = std::collections::HashMap::new();
                        payload.insert(
                            "content".to_string(),
                            qdrant_client::qdrant::Value {
                                kind: Some(qdrant_client::qdrant::value::Kind::StringValue(
                                    chunk.content,
                                )),
                            },
                        );
                        payload.insert(
                            "file_path".to_string(),
                            qdrant_client::qdrant::Value {
                                kind: Some(qdrant_client::qdrant::value::Kind::StringValue(
                                    chunk.file_path,
                                )),
                            },
                        );
                        payload.insert(
                            "start_line".to_string(),
                            qdrant_client::qdrant::Value {
                                kind: Some(qdrant_client::qdrant::value::Kind::IntegerValue(
                                    chunk.start_line as i64,
                                )),
                            },
                        );
                        payload.insert(
                            "end_line".to_string(),
                            qdrant_client::qdrant::Value {
                                kind: Some(qdrant_client::qdrant::value::Kind::IntegerValue(
                                    chunk.end_line as i64,
                                )),
                            },
                        );

                        if let Some(lang) = chunk.language {
                            payload.insert(
                                "language".to_string(),
                                qdrant_client::qdrant::Value {
                                    kind: Some(qdrant_client::qdrant::value::Kind::StringValue(
                                        lang,
                                    )),
                                },
                            );
                        }

                        PointStruct {
                            id: Some(chunk.id.into()),
                            vectors: Some(chunk.embedding.into()),
                            payload,
                        }
                    })
                    .collect();

                client
                    .upsert_points(UpsertPointsBuilder::new(&self.collection_name, points))
                    .await
                    .context("Failed to store embeddings")?;

                debug!("Successfully stored embeddings");
                Ok(())
            } else {
                debug!("Qdrant client not available, skipping storage");
                Ok(())
            }
        }

        #[cfg(not(feature = "semantic"))]
        {
            let _ = chunks;
            Ok(())
        }
    }

    /// Search for similar vectors
    pub async fn search(
        &self,
        query_embedding: Vec<f32>,
        limit: usize,
        filter: Option<Filter>,
    ) -> Result<Vec<SemanticSearchResult>> {
        #[cfg(feature = "semantic")]
        {
            if let Some(ref client) = self.client {
                debug!("Searching for {} similar vectors", limit);

                let search_params = SearchParamsBuilder::default().hnsw_ef(128).exact(false);

                let mut search_builder =
                    SearchPointsBuilder::new(&self.collection_name, query_embedding, limit as u64)
                        .with_payload(true)
                        .params(search_params);

                if let Some(filter) = filter {
                    search_builder = search_builder.filter(filter);
                }

                let results = client
                    .search_points(search_builder)
                    .await
                    .context("Failed to search points")?;

                let mut search_results = Vec::new();
                for result in results.result {
                    let payload = result.payload;
                    let file_path = payload
                        .get("file_path")
                        .and_then(|v| match &v.kind {
                            Some(qdrant_client::qdrant::value::Kind::StringValue(s)) => {
                                Some(s.clone())
                            },
                            _ => None,
                        })
                        .unwrap_or_default();

                    let content = payload
                        .get("content")
                        .and_then(|v| match &v.kind {
                            Some(qdrant_client::qdrant::value::Kind::StringValue(s)) => {
                                Some(s.clone())
                            },
                            _ => None,
                        })
                        .unwrap_or_default();

                    let start_line = payload
                        .get("start_line")
                        .and_then(|v| match &v.kind {
                            Some(qdrant_client::qdrant::value::Kind::IntegerValue(i)) => {
                                Some(*i as usize)
                            },
                            _ => None,
                        })
                        .unwrap_or(0);

                    let end_line = payload
                        .get("end_line")
                        .and_then(|v| match &v.kind {
                            Some(qdrant_client::qdrant::value::Kind::IntegerValue(i)) => {
                                Some(*i as usize)
                            },
                            _ => None,
                        })
                        .unwrap_or(0);

                    let language = payload.get("language").and_then(|v| match &v.kind {
                        Some(qdrant_client::qdrant::value::Kind::StringValue(s)) => Some(s.clone()),
                        _ => None,
                    });

                    search_results.push(SemanticSearchResult {
                        file_path,
                        content,
                        start_line,
                        end_line,
                        language,
                        score: result.score,
                    });
                }

                Ok(search_results)
            } else {
                debug!("Qdrant client not available");
                Ok(Vec::new())
            }
        }

        #[cfg(not(feature = "semantic"))]
        {
            let _ = (query_embedding, limit, filter);
            Ok(Vec::new())
        }
    }

    /// Check if Qdrant is available
    pub fn is_available(&self) -> bool {
        #[cfg(feature = "semantic")]
        {
            self.client.is_some()
        }
        #[cfg(not(feature = "semantic"))]
        {
            false
        }
    }

    /// Clear all data from the collection
    pub async fn clear_collection(&self) -> Result<()> {
        #[cfg(feature = "semantic")]
        {
            if let Some(ref client) = self.client {
                info!("Clearing collection '{}'", self.collection_name);
                client.delete_collection(&self.collection_name).await?;
                Self::init_collection(client, &self.collection_name).await?;
            }
        }
        Ok(())
    }
}

/// Represents a chunk of code with its embedding
#[derive(Debug, Clone)]
pub struct EmbeddedChunk {
    pub id: String,
    pub content: String,
    pub embedding: Vec<f32>,
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub language: Option<String>,
}

/// Result from semantic search
#[derive(Debug, Clone)]
pub struct SemanticSearchResult {
    pub file_path: String,
    pub content: String,
    pub start_line: usize,
    pub end_line: usize,
    pub language: Option<String>,
    pub score: f32,
}
