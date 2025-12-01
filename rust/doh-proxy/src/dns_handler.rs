use crate::doh_client::DohClient;
use std::sync::Arc;
use hickory_server::authority::MessageResponseBuilder;
use hickory_server::proto::op::{Header, ResponseCode};
use hickory_server::server::{Request, RequestHandler, ResponseHandler, ResponseInfo};

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
    async fn handle_request<R: ResponseHandler>(
        &self,
        request: &Request,
        mut response_handle: R,
    ) -> ResponseInfo {
        // Get the first query from the request
        let query = match request.queries().first() {
            Some(q) => q,
            None => {
                tracing::error!("Request has no queries");
                return Self::send_error_response(&mut response_handle, request, ResponseCode::FormErr).await;
            }
        };
        
        tracing::debug!("Received DNS query: {:?}", query);
        
        // Build a message from the request
        let mut query_msg = hickory_proto::op::Message::new();
        query_msg.set_id(request.id());
        query_msg.set_message_type(hickory_proto::op::MessageType::Query);
        query_msg.set_op_code(hickory_proto::op::OpCode::Query);
        query_msg.set_recursion_desired(true);
        
        // Create a proper Query from the LowerQuery
        let name = query.name().into();
        let hickory_query = hickory_proto::op::Query::query(
            name,
            query.query_type(),
        );
        query_msg.add_query(hickory_query);
        
        // Forward the request to DoH upstream
        match self.doh_client.query(&query_msg).await {
            Ok(doh_response) => {
                tracing::debug!("Received DoH response with {} answers", doh_response.answer_count());
                
                // Build response using MessageResponseBuilder
                let builder = MessageResponseBuilder::from_message_request(request);
                let response = builder.build(
                    doh_response.header().clone(),
                    doh_response.answers(),
                    doh_response.name_servers(),
                    &[],
                    doh_response.additionals(),
                );
                
                // Send the response back to the client
                match response_handle.send_response(response).await {
                    Ok(info) => info,
                    Err(e) => {
                        tracing::error!("Error sending response: {}", e);
                        Self::send_error_response(&mut response_handle, request, ResponseCode::ServFail).await
                    }
                }
            }
            Err(e) => {
                tracing::error!("DoH query failed: {}", e);
                Self::send_error_response(&mut response_handle, request, ResponseCode::ServFail).await
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
        let mut header = Header::response_from_request(request.header());
        header.set_response_code(response_code);
        
        let builder = MessageResponseBuilder::from_message_request(request);
        let response = builder.error_msg(&header, response_code);
        
        match response_handle.send_response(response).await {
            Ok(info) => info,
            Err(e) => {
                tracing::error!("Failed to send error response: {}", e);
                header.into()
            }
        }
    }
}
