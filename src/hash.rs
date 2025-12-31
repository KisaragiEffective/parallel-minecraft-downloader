use std::fmt::{Display, Formatter};
use serde::Deserialize;

#[derive(Deserialize, Eq, PartialEq, Copy, Clone)]
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
