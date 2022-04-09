use crate::*;

pub type DraftGroupIndex = u32;
pub type DraftIndex = u32;

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Clone))]
#[serde(crate = "near_sdk::serde")]
pub struct Draft {
    pub draft_group_id: DraftGroupIndex,
    pub lockup: Lockup,
}

#[derive(BorshDeserialize, BorshSerialize)]
pub struct DraftGroup {
    pub total_amount: Balance,
    pub draft_indices: HashSet<DraftIndex>,
}

impl DraftGroup {
    pub fn new() -> Self {
        Self {
            total_amount: 0,
            draft_indices: HashSet::new(),
        }
    }
}
