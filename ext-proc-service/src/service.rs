use std::sync::Arc;

use tokio::sync::RwLock;
use tonic::{Request, Response, Status};

use crate::mutation::MutationEngine;
use crate::proto;

// ---------------------------------------------------------------------------
// ProtoMutationService implementation
// ---------------------------------------------------------------------------

pub struct ProtoMutationServiceImpl {
    engine: Arc<RwLock<MutationEngine>>,
}

impl ProtoMutationServiceImpl {
    pub fn new(engine: Arc<RwLock<MutationEngine>>) -> Self {
        Self { engine }
    }
}

#[tonic::async_trait]
impl proto::proto_mutation_service_server::ProtoMutationService for ProtoMutationServiceImpl {
    async fn process_message(
        &self,
        request: Request<proto::ProcessMessageRequest>,
    ) -> Result<Response<proto::ProcessMessageResponse>, Status> {
        let req = request.into_inner();

        tracing::debug!(
            service = %req.service_name,
            method = %req.method_name,
            direction = req.direction,
            body_size = req.raw_body.len(),
            num_fields = req.decoded_fields.len(),
            "processing message"
        );

        let engine = self.engine.read().await;
        let response = engine.evaluate(&req);

        tracing::debug!(
            action = response.action,
            num_mutations = response.mutations.len(),
            "returning response"
        );

        Ok(Response::new(response))
    }
}

// ---------------------------------------------------------------------------
// MutationAdminService implementation
// ---------------------------------------------------------------------------

pub struct AdminServiceImpl {
    engine: Arc<RwLock<MutationEngine>>,
}

impl AdminServiceImpl {
    pub fn new(engine: Arc<RwLock<MutationEngine>>) -> Self {
        Self { engine }
    }
}

#[tonic::async_trait]
impl proto::mutation_admin_service_server::MutationAdminService for AdminServiceImpl {
    async fn list_rules(
        &self,
        _request: Request<proto::ListRulesRequest>,
    ) -> Result<Response<proto::ListRulesResponse>, Status> {
        let engine = self.engine.read().await;
        let rules = engine.list_rules();
        tracing::info!(count = rules.len(), "listing rules");
        Ok(Response::new(proto::ListRulesResponse { rules }))
    }

    async fn add_rule(
        &self,
        request: Request<proto::AddRuleRequest>,
    ) -> Result<Response<proto::AddRuleResponse>, Status> {
        let req = request.into_inner();
        let rule = req
            .rule
            .ok_or_else(|| Status::invalid_argument("rule is required"))?;

        let rule_name = rule.name.clone();
        let mut engine = self.engine.write().await;
        let rule_id = engine.add_rule(rule);

        tracing::info!(rule_id = %rule_id, name = %rule_name, "added rule");
        Ok(Response::new(proto::AddRuleResponse { rule_id }))
    }

    async fn remove_rule(
        &self,
        request: Request<proto::RemoveRuleRequest>,
    ) -> Result<Response<proto::RemoveRuleResponse>, Status> {
        let req = request.into_inner();
        let mut engine = self.engine.write().await;
        let found = engine.remove_rule(&req.rule_id);

        tracing::info!(rule_id = %req.rule_id, found = found, "removed rule");
        Ok(Response::new(proto::RemoveRuleResponse { found }))
    }

    async fn update_rule(
        &self,
        request: Request<proto::UpdateRuleRequest>,
    ) -> Result<Response<proto::UpdateRuleResponse>, Status> {
        let req = request.into_inner();
        let rule = req
            .rule
            .ok_or_else(|| Status::invalid_argument("rule is required"))?;

        let rule_id = rule.id.clone();
        let mut engine = self.engine.write().await;
        let found = engine.update_rule(rule);

        tracing::info!(rule_id = %rule_id, found = found, "updated rule");
        Ok(Response::new(proto::UpdateRuleResponse { found }))
    }
}
