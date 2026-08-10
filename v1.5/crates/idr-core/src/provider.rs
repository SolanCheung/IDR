use crate::{HostModelProviderError, HostModelRequestV1, HostModelResultV1};
use async_trait::async_trait;

#[async_trait]
pub trait HostModelProvider: Send + Sync {
    async fn infer(
        &self,
        request: HostModelRequestV1,
    ) -> Result<HostModelResultV1, HostModelProviderError>;
}

