use crate::message::{RequestMessage, ResponseMessage};
use crate::transport::OmniRequestHandler;
use crate::OmniError;
use async_trait::async_trait;
use std::collections::BTreeMap;
use std::fmt::Formatter;
use std::sync::Arc;

#[derive(Clone, Default)]
pub struct ModuleRequestHandler {
    handlers: BTreeMap<String, Arc<dyn OmniRequestHandler>>,
}

impl ModuleRequestHandler {
    pub fn empty() -> Self {
        Default::default()
    }

    pub fn with_method<NS, H>(mut self, method_name: NS, handler: H) -> Self
    where
        NS: ToString,
        H: OmniRequestHandler + 'static,
    {
        self.handlers
            .insert(method_name.to_string(), Arc::new(handler));
        self
    }
}

impl std::fmt::Debug for ModuleRequestHandler {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("ModuleRequestHandler ")?;
        let mut list = f.debug_list();
        for (ns, _) in &self.handlers {
            list.entry(ns);
        }
        list.finish()
    }
}

#[async_trait]
impl OmniRequestHandler for ModuleRequestHandler {
    fn validate(&self, message: &RequestMessage) -> Result<(), OmniError> {
        let method = message.method.as_str();
        if let Some(h) = self.handlers.get(method) {
            h.validate(message)
        } else {
            Err(OmniError::invalid_method_name(method.to_string()))
        }
    }

    async fn execute(&self, message: &RequestMessage) -> Result<ResponseMessage, OmniError> {
        let method = message.method.as_str();
        if let Some(h) = self.handlers.get(method) {
            h.execute(message).await
        } else {
            Err(OmniError::invalid_method_name(method.to_string()))
        }
    }
}
