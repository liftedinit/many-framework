use crate::message::error::OmniErrorCode;
use crate::message::{RequestMessage, ResponseMessage};
use crate::transport::OmniRequestHandler;
use crate::OmniError;
use async_trait::async_trait;
use std::collections::BTreeMap;
use std::fmt::{Debug, Formatter};
use std::sync::Arc;

#[derive(Clone, Default)]
pub struct NamespacedRequestHandler {
    namespaces: BTreeMap<String, Arc<dyn OmniRequestHandler>>,
}

impl NamespacedRequestHandler {
    pub fn empty() -> Self {
        Default::default()
    }

    pub fn new<H: OmniRequestHandler + 'static>(default_handler: H) -> Self {
        let mut handler = Self::empty();
        handler.with_namespace("", default_handler);
        handler
    }

    pub fn with_namespace<NS, H>(&mut self, namespace: NS, handler: H) -> &mut Self
    where
        NS: ToString,
        H: OmniRequestHandler + 'static,
    {
        self.namespaces
            .insert(namespace.to_string(), Arc::new(handler));
        self
    }

    pub fn resolve_namespace<'a>(
        &self,
        method_name: &'a str,
    ) -> Option<(&'a str, &dyn OmniRequestHandler)> {
        let (namespace, method_name) = method_name.split_once(".").unwrap_or(("", method_name));

        self.namespaces
            .get(namespace)
            .map(|handler| ((method_name, handler.as_ref())))
    }
}

impl Debug for NamespacedRequestHandler {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("NamespacedRequestHandler ")?;
        let mut list = f.debug_list();
        for (ns, _) in &self.namespaces {
            list.entry(&ns);
        }
        list.finish()
    }
}

#[async_trait]
impl OmniRequestHandler for NamespacedRequestHandler {
    fn validate(&self, message: &RequestMessage) -> Result<(), OmniError> {
        let method = message.method.as_str();
        if let Some((m, h)) = self.resolve_namespace(method) {
            let mut message = message.clone();
            h.validate(message.with_method(m.to_string()))
        } else {
            Err(OmniError::invalid_method_name(method.to_string()))
        }
    }

    async fn execute(&self, message: &RequestMessage) -> Result<ResponseMessage, OmniError> {
        let method = message.method.as_str();
        let maybe_method = self.resolve_namespace(method);

        match maybe_method {
            None => Err(OmniError::invalid_method_name(method.to_string())),
            Some((method, handler)) => match handler.execute(message).await {
                Err(OmniError {
                    code: OmniErrorCode::InvalidMethodName,
                    ..
                }) => Err(OmniError::invalid_method_name(method.to_string())),
                x => x,
            },
        }
    }
}
