use crate::doh_client::DohClient;
use hickory_server::net::runtime::Time;
use hickory_server::proto::op::{Header, HeaderCounts, Metadata, ResponseCode};
use hickory_server::server::{Request, RequestHandler, ResponseHandler, ResponseInfo};
use hickory_server::zone_handler::MessageResponseBuilder;
use std::sync::Arc;

pub struct DnsHandler {
    doh_client: Arc<DohClient>,
}

impl DnsHandler {
    pub fn new(doh_client: Arc<DohClient>) -> Self {
        Self { doh_client }
    }
}

#[async_trait::async_trait]
impl RequestHandler for DnsHandler {
    async fn handle_request<R: ResponseHandler, T: Time>(
        &self,
        request: &Request,
        mut response_handle: R,
    ) -> ResponseInfo {
        // Get the first query from the request
        let query = match request.queries.queries().first() {
            Some(q) => q,
            None => {
                tracing::error!("Request has no queries");
                return Self::send_error_response(
                    &mut response_handle,
                    request,
                    ResponseCode::FormErr,
                )
                .await;
            }
        };

        tracing::debug!("Received DNS query: {:?}", query);

        // Log DNSSEC request status
        if let Some(edns) = &request.edns {
            tracing::debug!(
                "EDNS present: DO bit={}, buffer_size={}",
                edns.flags().dnssec_ok,
                edns.max_payload()
            );
        }

        // Build a message from the request, preserving important flags
        let mut query_msg = hickory_proto::op::Message::new(
            request.metadata.id,
            hickory_proto::op::MessageType::Query,
            request.metadata.op_code,
        );
        query_msg.metadata.recursion_desired = request.metadata.recursion_desired;

        // Preserve DNSSEC-related flags
        query_msg.metadata.checking_disabled = request.metadata.checking_disabled;
        query_msg.metadata.authentic_data = request.metadata.authentic_data;

        // Create a proper Query from the LowerQuery
        let name = query.name().into();
        let hickory_query = hickory_proto::op::Query::query(name, query.query_type());
        query_msg.add_query(hickory_query);

        // Preserve EDNS settings (including DNSSEC OK flag)
        if let Some(edns) = &request.edns {
            let mut edns_builder = hickory_proto::op::Edns::new();
            edns_builder.set_max_payload(edns.max_payload());
            edns_builder.set_version(edns.version());

            // CRITICAL: Set the DNSSEC OK (DO) bit if present in request
            // This tells upstream servers to include DNSSEC records in their response
            edns_builder.set_dnssec_ok(edns.flags().dnssec_ok);

            // Copy EDNS options
            for (_code, opt) in edns.options().as_ref() {
                edns_builder.options_mut().insert(opt.clone());
            }

            query_msg.set_edns(edns_builder);
        }

        // Forward the request to DoH upstream
        match self.doh_client.query(&query_msg).await {
            Ok(doh_response) => {
                // Log DNSSEC-related information for debugging
                tracing::debug!(
                    "DoH response: answers={}, authority={}, additional={}, AD={}, CD={}",
                    doh_response.answers.len(),
                    doh_response.authorities.len(),
                    doh_response.additionals.len(),
                    doh_response.metadata.authentic_data,
                    doh_response.metadata.checking_disabled
                );

                // Log DNSSEC records if present
                if tracing::enabled!(tracing::Level::DEBUG) {
                    for record in doh_response.answers.iter() {
                        if matches!(
                            record.record_type(),
                            hickory_proto::rr::RecordType::RRSIG
                                | hickory_proto::rr::RecordType::DNSKEY
                                | hickory_proto::rr::RecordType::DS
                                | hickory_proto::rr::RecordType::NSEC
                                | hickory_proto::rr::RecordType::NSEC3
                        ) {
                            tracing::debug!(
                                "  DNSSEC record in answer: {:?}",
                                record.record_type()
                            );
                        }
                    }
                    for record in doh_response.additionals.iter() {
                        tracing::debug!("  Additional record: {:?}", record.record_type());
                    }
                }

                // Forward the complete upstream response preserving all DNSSEC information
                // Update only the ID to match the request
                let mut metadata = doh_response.metadata;
                metadata.id = request.metadata.id;

                // Convert the Message into a MessageResponse that can be sent
                // We use from_message_request to get the proper response structure,
                // but then build with the complete upstream data
                let builder = MessageResponseBuilder::from_message_request(request);

                // Build response preserving everything from upstream including EDNS/DNSSEC
                let response = builder.build(
                    metadata,
                    &doh_response.answers,
                    &doh_response.authorities,
                    &[], // SOA: authority/SOA records are already included in authorities above
                    &doh_response.additionals,
                );

                tracing::debug!(
                    "Sending response with AD={} CD={}",
                    metadata.authentic_data,
                    metadata.checking_disabled
                );

                // Send the response back to the client
                match response_handle.send_response(response).await {
                    Ok(info) => info,
                    Err(e) => {
                        tracing::error!("Error sending response: {}", e);
                        Self::send_error_response(
                            &mut response_handle,
                            request,
                            ResponseCode::ServFail,
                        )
                        .await
                    }
                }
            }
            Err(e) => {
                tracing::error!("DoH query failed: {}", e);
                Self::send_error_response(&mut response_handle, request, ResponseCode::ServFail)
                    .await
            }
        }
    }
}

impl DnsHandler {
    async fn send_error_response<R: ResponseHandler>(
        response_handle: &mut R,
        request: &Request,
        response_code: ResponseCode,
    ) -> ResponseInfo {
        let builder = MessageResponseBuilder::from_message_request(request);
        let response = builder.error_msg(&request.metadata, response_code);

        match response_handle.send_response(response).await {
            Ok(info) => info,
            Err(e) => {
                tracing::error!("Failed to send error response: {}", e);
                let mut metadata = Metadata::response_from_request(&request.metadata);
                metadata.response_code = response_code;
                Header {
                    metadata,
                    counts: HeaderCounts::default(),
                }
                .into()
            }
        }
    }
}
