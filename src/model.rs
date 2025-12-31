use serde::Deserialize;
use url::Url;
use std::collections::HashMap;
use std::convert::Infallible;
use std::str::FromStr;
use crate::hash::Sha1Hash;

#[derive(Deserialize)]
pub struct PartialVersionManifestRoot {
    pub versions: Vec<VersionMetadata>
}

#[derive(Deserialize)]
pub struct VersionMetadata {
    pub id: VersionIdentifier,
    pub url: Url
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetailedVersionMetadata {
    pub asset_index: AssetIndexPointee
}

#[derive(Deserialize)]
pub struct AssetIndexPointee {
    pub url: Url
}

#[derive(Deserialize)]
pub struct AssetMappingRoot {
    pub objects: HashMap<String, AssetMappingValue>,
}

#[derive(Deserialize)]
pub struct AssetMappingValue {
    pub hash: Sha1Hash,
    pub size: usize,
}

#[derive(Deserialize, Eq, PartialEq, Clone, Debug)]
pub struct VersionIdentifier(pub String);

impl FromStr for VersionIdentifier {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.to_owned()))
    }
}
