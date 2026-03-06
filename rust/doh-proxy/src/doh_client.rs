use anyhow::{anyhow, Result};
use hickory_proto::op::Message;
use hickory_proto::serialize::binary::{BinDecodable, BinEncodable};
use moka::future::Cache;
use moka::Expiry;
use reqwest::Client;
use std::time::Duration;

// Cache key: (query name, query type, query class, dnssec_ok)
type CacheKey = (String, u16, u16, bool);

#[derive(Clone)]
struct CachedResponse {
    message: Message,
    ttl: Duration,
}

struct DnsExpiry;

impl Expiry<CacheKey, CachedResponse> for DnsExpiry {
    fn expire_after_create(
        &self,
        _key: &CacheKey,
        value: &CachedResponse,
        _created_at: std::time::Instant,
    ) -> Option<Duration> {
        Some(value.ttl)
    }
}

pub struct DohClient {
    client: Client,
    servers: Vec<String>,
    current_server_index: std::sync::atomic::AtomicUsize,
    cache: Option<Cache<CacheKey, CachedResponse>>,
}

impl DohClient {
    pub fn new(servers: Vec<String>, timeout_secs: u64, cache_size: u64) -> Result<Self> {
        let client = Client::builder()
            .use_native_tls()
            .timeout(Duration::from_secs(timeout_secs))
            .build()?;

        // Create cache only if cache_size > 0
        let cache = if cache_size > 0 {
            Some(
                Cache::builder()
                    .max_capacity(cache_size)
                    .expire_after(DnsExpiry)
                    .build(),
            )
        } else {
            None
        };

        Ok(Self {
            client,
            servers,
            current_server_index: std::sync::atomic::AtomicUsize::new(0),
            cache,
        })
    }

    /// Query a DoH server with the DNS message (with caching)
    pub async fn query(&self, request: &Message) -> Result<Message> {
        // If caching is disabled, directly query upstream
        if self.cache.is_none() {
            return self.query_upstream(request).await;
        }

        // Extract query information for cache key
        if let Some(query) = request.queries().first() {
            let dnssec_ok = request
                .extensions()
                .as_ref()
                .map(|edns| edns.flags().dnssec_ok)
                .unwrap_or(false);
            let cache_key = (
                query.name().to_string().to_lowercase(),
                query.query_type().into(),
                query.query_class().into(),
                dnssec_ok,
            );

            // Check cache first (safe to unwrap since we checked is_none above)
            let cache = self.cache.as_ref().unwrap();
            if let Some(cached) = cache.get(&cache_key).await {
                tracing::debug!("Cache HIT for {}", query.name());
                let mut response = cached.message.clone();
                response.set_id(request.id());
                return Ok(response);
            }

            tracing::debug!("Cache MISS for {}", query.name());

            // Query upstream
            let response = self.query_upstream(request).await?;

            // Calculate TTL and cache the response
            let ttl = self.calculate_ttl(&response);
            if ttl > 0 {
                cache
                    .insert(
                        cache_key,
                        CachedResponse {
                            message: response.clone(),
                            ttl: Duration::from_secs(ttl as u64),
                        },
                    )
                    .await;
                tracing::debug!("Cached response for {} with TTL {}s", query.name(), ttl);
            }

            Ok(response)
        } else {
            // No query in request, just forward
            self.query_upstream(request).await
        }
    }

    /// Calculate minimum TTL from all records in the response
    fn calculate_ttl(&self, response: &Message) -> u32 {
        let mut min_ttl = u32::MAX;

        // Check all record sections
        for record in response
            .answers()
            .iter()
            .chain(response.name_servers().iter())
            .chain(response.additionals().iter())
        {
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
            let server_index = (self
                .current_server_index
                .load(std::sync::atomic::Ordering::Relaxed)
                + i)
                % self.servers.len();
            let server = &self.servers[server_index];

            match self.query_server(server, &request_bytes).await {
                Ok(response) => {
                    // Update current server on success
                    self.current_server_index
                        .store(server_index, std::sync::atomic::Ordering::Relaxed);
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
        let response = self
            .client
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

#[cfg(test)]
mod tests {
    use super::*;
    use hickory_proto::op::{MessageType, OpCode, Query};
    use hickory_proto::rr::rdata::A;
    use hickory_proto::rr::{Name, RData, Record, RecordType};
    use std::net::Ipv4Addr;
    use std::str::FromStr;
    use wiremock::matchers::{header, method};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn make_query(name: &str, rtype: RecordType) -> Message {
        let mut msg = Message::new();
        msg.set_id(1234);
        msg.set_message_type(MessageType::Query);
        msg.set_op_code(OpCode::Query);
        msg.set_recursion_desired(true);
        msg.add_query(Query::query(Name::from_str(name).unwrap(), rtype));
        msg
    }

    fn make_response(request: &Message, records: Vec<Record>, ttl: u32) -> Message {
        let mut msg = Message::new();
        msg.set_id(request.id());
        msg.set_message_type(MessageType::Response);
        msg.set_op_code(OpCode::Query);
        msg.set_recursion_desired(true);
        msg.set_recursion_available(true);

        for mut record in records {
            record.set_ttl(ttl);
            msg.add_answer(record);
        }
        msg
    }

    fn a_record(name: &str, ip: Ipv4Addr) -> Record {
        Record::from_rdata(Name::from_str(name).unwrap(), 300, RData::A(A(ip)))
    }

    // --- calculate_ttl tests ---

    #[test]
    fn calculate_ttl_uses_minimum_across_sections() {
        let client = DohClient::new(vec!["https://dummy".into()], 5, 0).unwrap();

        let mut msg = Message::new();
        msg.add_answer(Record::from_rdata(
            Name::from_str("a.example.").unwrap(),
            600,
            RData::A(A(Ipv4Addr::LOCALHOST)),
        ));
        msg.add_name_server(Record::from_rdata(
            Name::from_str("ns.example.").unwrap(),
            120,
            RData::A(A(Ipv4Addr::LOCALHOST)),
        ));

        assert_eq!(client.calculate_ttl(&msg), 120);
    }

    #[test]
    fn calculate_ttl_caps_at_one_hour() {
        let client = DohClient::new(vec!["https://dummy".into()], 5, 0).unwrap();

        let mut msg = Message::new();
        msg.add_answer(Record::from_rdata(
            Name::from_str("a.example.").unwrap(),
            86400, // 24 hours
            RData::A(A(Ipv4Addr::LOCALHOST)),
        ));

        assert_eq!(client.calculate_ttl(&msg), 3600);
    }

    #[test]
    fn calculate_ttl_default_when_no_records() {
        let client = DohClient::new(vec!["https://dummy".into()], 5, 0).unwrap();
        let msg = Message::new();
        assert_eq!(client.calculate_ttl(&msg), 300);
    }

    // --- constructor tests ---

    #[test]
    fn new_with_cache_disabled() {
        let client = DohClient::new(vec!["https://dummy".into()], 5, 0).unwrap();
        assert!(client.cache.is_none());
    }

    #[test]
    fn new_with_cache_enabled() {
        let client = DohClient::new(vec!["https://dummy".into()], 5, 100).unwrap();
        assert!(client.cache.is_some());
    }

    // --- integration tests with mock server ---

    #[tokio::test]
    async fn query_server_returns_dns_response() {
        let server = MockServer::start().await;
        let query = make_query("example.com.", RecordType::A);
        let response = make_response(
            &query,
            vec![a_record("example.com.", Ipv4Addr::new(93, 184, 216, 34))],
            300,
        );
        let response_bytes = response.to_bytes().unwrap();

        Mock::given(method("POST"))
            .and(header("Content-Type", "application/dns-message"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_bytes(response_bytes)
                    .append_header("Content-Type", "application/dns-message"),
            )
            .mount(&server)
            .await;

        let client = DohClient::new(vec![server.uri() + "/dns-query"], 5, 0).unwrap();
        let result = client.query(&query).await.unwrap();

        assert_eq!(result.answer_count(), 1);
        assert_eq!(result.answers()[0].record_type(), RecordType::A);
    }

    #[tokio::test]
    async fn query_server_returns_error_on_http_failure() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;

        let client = DohClient::new(vec![server.uri() + "/dns-query"], 5, 0).unwrap();
        let query = make_query("example.com.", RecordType::A);
        let result = client.query(&query).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("503"));
    }

    #[tokio::test]
    async fn query_fails_over_to_next_server() {
        let bad_server = MockServer::start().await;
        let good_server = MockServer::start().await;

        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&bad_server)
            .await;

        let query = make_query("example.com.", RecordType::A);
        let response = make_response(
            &query,
            vec![a_record("example.com.", Ipv4Addr::new(1, 2, 3, 4))],
            300,
        );

        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_bytes(response.to_bytes().unwrap())
                    .append_header("Content-Type", "application/dns-message"),
            )
            .mount(&good_server)
            .await;

        let client = DohClient::new(
            vec![
                bad_server.uri() + "/dns-query",
                good_server.uri() + "/dns-query",
            ],
            5,
            0,
        )
        .unwrap();

        let result = client.query(&query).await.unwrap();
        assert_eq!(result.answer_count(), 1);
    }

    #[tokio::test]
    async fn query_caches_response_and_serves_from_cache() {
        let server = MockServer::start().await;
        let query = make_query("cached.example.com.", RecordType::A);
        let response = make_response(
            &query,
            vec![a_record(
                "cached.example.com.",
                Ipv4Addr::new(10, 0, 0, 1),
            )],
            300,
        );

        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_bytes(response.to_bytes().unwrap())
                    .append_header("Content-Type", "application/dns-message"),
            )
            .expect(1) // Should only be called once; second query hits cache
            .mount(&server)
            .await;

        let client = DohClient::new(vec![server.uri() + "/dns-query"], 5, 100).unwrap();

        // First query: cache miss, hits upstream
        let r1 = client.query(&query).await.unwrap();
        assert_eq!(r1.answer_count(), 1);

        // Second query: should be served from cache
        let mut query2 = query.clone();
        query2.set_id(5678);
        let r2 = client.query(&query2).await.unwrap();
        assert_eq!(r2.answer_count(), 1);
        assert_eq!(r2.id(), 5678); // ID should match the new request
    }

    #[tokio::test]
    async fn query_with_no_cache_always_hits_upstream() {
        let server = MockServer::start().await;
        let query = make_query("nocache.example.com.", RecordType::A);
        let response = make_response(
            &query,
            vec![a_record(
                "nocache.example.com.",
                Ipv4Addr::new(10, 0, 0, 2),
            )],
            300,
        );

        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_bytes(response.to_bytes().unwrap())
                    .append_header("Content-Type", "application/dns-message"),
            )
            .expect(2) // Both queries should hit upstream
            .mount(&server)
            .await;

        let client = DohClient::new(vec![server.uri() + "/dns-query"], 5, 0).unwrap();

        client.query(&query).await.unwrap();
        client.query(&query).await.unwrap();
    }
}
