use serde::Deserialize;
use url::Url;
use std::collections::HashMap;
use std::convert::Infallible;
use std::fmt::{Display, Formatter};
use std::str::FromStr;

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

#[derive(Deserialize, Eq, PartialEq)]
pub struct Sha1Hash(pub sha1_smol::Digest);

impl Sha1Hash {
    pub fn human_readable(&self) -> String {
        self.0.to_string()
    }
}

impl Display for Sha1Hash {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.human_readable())
    }
}
