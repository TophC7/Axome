use crate::{errors::ApiError, services::extensions::ExtensionService, Host};
use axome_protocol::extensions::{ExtensionCatalog, ExtensionGrammar};

impl Host {
    pub fn extensions_get_catalog(&self) -> Result<ExtensionCatalog, ApiError> {
        Ok(ExtensionService::current().catalog.clone())
    }

    pub fn extensions_get_grammars(&self) -> Result<Vec<ExtensionGrammar>, ApiError> {
        Ok(ExtensionService::current().grammars.clone())
    }

    pub fn extensions_reload(&self) -> Result<(), ApiError> {
        ExtensionService::reload();
        Ok(())
    }
}
