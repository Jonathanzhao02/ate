use serde::{Serialize, Deserialize, de::DeserializeOwned};
use super::crypto::*;
use super::header::*;
use super::signature::MetaSignature;

pub trait OtherMetadata
where Self: Serialize + DeserializeOwned + std::fmt::Debug + Default + Clone + Sized
{
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MetaAuthorization
{
    pub allow_read: Vec<Hash>,
    pub allow_write: Vec<Hash>,
    pub implicit_authority: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MetaTree
{
    pub parent: PrimaryKey,
    pub inherit_read: bool,
    pub inherit_write: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum CoreMetadata
{
    None,
    Data(PrimaryKey),
    Tombstone(PrimaryKey),
    Authorization(MetaAuthorization),
    InitializationVector(InitializationVector),
    PublicKey(PublicKey),
    EncryptedPrivateKey(EncryptedPrivateKey),
    EncryptedEncryptionKey(EncryptKey),
    Tree(MetaTree),
    Signature(MetaSignature),
    Author(String),
}

impl Default for CoreMetadata {
    fn default() -> Self {
        CoreMetadata::None
    }
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct NoAdditionalMetadata { }
impl OtherMetadata for NoAdditionalMetadata { }

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct MetadataExt<M>
{
    pub core: Vec<CoreMetadata>,
    pub other: M,
}

#[allow(dead_code)]
pub type DefaultMetadata = MetadataExt<NoAdditionalMetadata>;

impl<M> MetadataExt<M>
{
    pub fn get_authorization(&self) -> Option<&MetaAuthorization>
    {
        for core in &self.core {
            match core {
                CoreMetadata::Authorization(a) => {
                    return Some(a);
                },
                _ => {}
            }
        }
        
        None
    }

    pub fn needs_signature(&self) -> bool
    {
        for core in &self.core {
            match core {
                CoreMetadata::PublicKey(_) => {},
                CoreMetadata::Signature(_) => {},
                CoreMetadata::EncryptedPrivateKey(_) => {},
                CoreMetadata::EncryptedEncryptionKey(_) => {},                
                _ => { return true; }
            }
        }

        false
    }
}