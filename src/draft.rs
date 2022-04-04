use crate::*;

pub type DraftIndex = u32;

#[derive(BorshDeserialize, BorshSerialize)]
pub struct Draft {
    pub lockup: Lockup,
}

#[derive(BorshDeserialize, BorshSerialize)]
pub struct DraftGroup {
    pub draft_indices: HashSet<DraftIndex>,
}
