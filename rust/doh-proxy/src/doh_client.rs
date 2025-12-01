use anyhow::{anyhow, Result};
use moka::future::Cache;
use reqwest::Client;
use std::time::Duration;
use hickory_proto::op::Message;
use hickory_proto::serialize::binary::{BinDecodable, BinEncodable};

// Cache key: (query name, query type)
type CacheKey = (String, u16);

pub struct DohClient {
    client: Client,
    servers: Vec<String>,
    current_server_index: std::sync::atomic::AtomicUsize,
    cache: Cache<CacheKey, Message>,
}

impl DohClient {
    pub fn new(servers: Vec<String>, timeout_secs: u64, cache_size: u64) -> Result<Self> {
        let client = Client::builder()
            .use_native_tls()
            .timeout(Duration::from_secs(timeout_secs))
            .build()?;
        
        // Create cache with configurable size
        let cache = Cache::builder()
            .max_capacity(cache_size)
            .build();
        
        Ok(Self {
            client,
            servers,
            current_server_index: std::sync::atomic::AtomicUsize::new(0),
            cache,
        })
    }
    
    /// Query a DoH server with the DNS message (with caching)
    pub async fn query(&self, request: &Message) -> Result<Message> {
        // Extract query information for cache key
        if let Some(query) = request.queries().first() {
            let cache_key = (
                query.name().to_string().to_lowercase(),
                query.query_type().into(),
            );
            
            // Check cache first
            if let Some(cached_response) = self.cache.get(&cache_key).await {
                tracing::debug!("Cache HIT for {}", query.name());
                // Clone and update the message ID to match the request
                let mut response = cached_response.clone();
                response.set_id(request.id());
                return Ok(response);
            }
            
            tracing::debug!("Cache MISS for {}", query.name());
            
            // Query upstream
            match self.query_upstream(request).await {
                Ok(response) => {
                    // Calculate TTL from response records
                    let ttl = self.calculate_ttl(&response);
                    
                    if ttl > 0 {
                        // Store in cache with TTL
                        let cache_clone = self.cache.clone();
                        let key_clone = cache_key.clone();
                        let response_clone = response.clone();
                        
                        tokio::spawn(async move {
                            cache_clone.insert(key_clone, response_clone).await;
                        });
                        
                        // Schedule cache invalidation after TTL
                        let cache_for_expiry = self.cache.clone();
                        tokio::spawn(async move {
                            tokio::time::sleep(Duration::from_secs(ttl as u64)).await;
                            cache_for_expiry.invalidate(&cache_key).await;
                        });
                        
                        tracing::debug!("Cached response for {} with TTL {}s", query.name(), ttl);
                    }
                    
                    Ok(response)
                }
                Err(e) => Err(e),
            }
        } else {
            // No query in request, just forward
            self.query_upstream(request).await
        }
    }
    
    /// Calculate minimum TTL from all records in the response
    fn calculate_ttl(&self, response: &Message) -> u32 {
        let mut min_ttl = u32::MAX;
        
        // Check all record sections
        for record in response.answers().iter()
            .chain(response.name_servers().iter())
            .chain(response.additionals().iter()) {
            min_ttl = min_ttl.min(record.ttl());
        }
        
        // Use a reasonable default if no records or TTL is too high
        if min_ttl == u32::MAX {
            300 // 5 minutes default
        } else {
            min_ttl.min(3600) // Cap at 1 hour
        }
    }
    
    /// Query upstream DoH servers without caching
    async fn query_upstream(&self, request: &Message) -> Result<Message> {
        // Serialize the DNS message to wire format
        let request_bytes = request.to_bytes()?;
        
        // Try each server in order until one succeeds
        let mut last_error = None;
        
        for i in 0..self.servers.len() {
            let server_index = (self.current_server_index.load(std::sync::atomic::Ordering::Relaxed) + i) 
                % self.servers.len();
            let server = &self.servers[server_index];
            
            match self.query_server(server, &request_bytes).await {
                Ok(response) => {
                    // Update current server on success
                    self.current_server_index.store(server_index, std::sync::atomic::Ordering::Relaxed);
                    return Ok(response);
                }
                Err(e) => {
                    tracing::warn!("DoH query to {} failed: {}", server, e);
                    last_error = Some(e);
                }
            }
        }
        
        Err(last_error.unwrap_or_else(|| anyhow!("No DoH servers available")))
    }
    
    async fn query_server(&self, server: &str, request_bytes: &[u8]) -> Result<Message> {
        // Use POST method with DNS wireformat in body
        let response = self.client
            .post(server)
            .header("Content-Type", "application/dns-message")
            .header("Accept", "application/dns-message")
            .body(request_bytes.to_vec())
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Err(anyhow!("DoH server returned status: {}", response.status()));
        }
        
        let response_bytes = response.bytes().await?;
        let dns_response = Message::from_bytes(&response_bytes)?;
        
        Ok(dns_response)
    }
}
