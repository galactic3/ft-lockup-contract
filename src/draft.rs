use crate::*;

pub type DraftGroupIndex = u32;
pub type DraftIndex = u32;

#[derive(BorshDeserialize, BorshSerialize)]
pub struct Draft {
    pub lockup: Lockup,
}

#[derive(BorshDeserialize, BorshSerialize)]
pub struct DraftGroup {
    pub draft_indices: HashSet<DraftIndex>,
}

impl DraftGroup {
    pub fn new() -> Self {
        Self {
            draft_indices: HashSet::new(),
        }
    }
}
