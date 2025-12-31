use crate::hash::Sha1Hash;

pub struct ConsoleMessage {
    pub item_index: usize,
    pub hash: Sha1Hash,
    pub state: State,
}

pub enum State {
    Checking,
    Cached,
    Processing,
    Done,
    CorruptedMetadata(CorruptedMetadataAction),
}

pub enum CorruptedMetadataAction {
    ForciblyContinued,
    Skipped { actual_hash: Sha1Hash },
}
