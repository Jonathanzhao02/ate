use super::meta::*;
use super::crypto::*;
use super::event::*;
use super::signature::MetaSignature;
use super::error::*;

#[derive(Debug)]
pub enum ValidationResult {
    Deny,
    Allow,
    #[allow(dead_code)]
    Abstain,
}

pub trait EventValidator: Send + Sync
{
    fn validate(&self, _header: &EventHeader) -> Result<ValidationResult, ValidationError> {
        Ok(ValidationResult::Abstain)
    }

    fn clone_validator(&self) -> Box<dyn EventValidator>;
}

#[derive(Default, Clone)]
pub struct RubberStampValidator {   
}

impl EventValidator
for RubberStampValidator
{
    fn clone_validator(&self) -> Box<dyn EventValidator> {
        Box::new(self.clone())
    }

    #[allow(unused_variables)]
    fn validate(&self, _header: &EventHeader) -> Result<ValidationResult, ValidationError>
    {
        Ok(ValidationResult::Allow)
    }
}

#[derive(Debug, Clone)]
pub struct StaticSignatureValidator {
    #[allow(dead_code)]
    pk: PublicSignKey,
}

impl StaticSignatureValidator
{
    #[allow(dead_code)]
    pub fn new(key: &PublicSignKey) -> StaticSignatureValidator {
        StaticSignatureValidator {
            pk: key.clone(),
        }
    }
}

impl EventValidator
for StaticSignatureValidator
{
    fn clone_validator(&self) -> Box<dyn EventValidator> {
        Box::new(self.clone())
    }
    
    #[allow(unused_variables)]
    fn validate(&self, _header: &EventHeader) -> Result<ValidationResult, ValidationError>
    {
        Ok(ValidationResult::Allow)
    }
}

impl Metadata
{
    #[allow(dead_code)]
    pub fn add_signature(&mut self, _sig: MetaSignature) {
    }

    #[allow(dead_code)]
    pub fn get_signature(&self) -> Option<MetaSignature> {
        None
    }
}