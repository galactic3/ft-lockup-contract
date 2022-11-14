use crate::*;
use near_contract_standards::upgrade::Ownable;

#[derive(BorshDeserialize, BorshSerialize)]
pub struct OldContract {
    pub token_account_id: TokenAccountId,
    pub lockups: Vector<Lockup>,
    pub account_lockups: LookupMap<AccountId, HashSet<LockupIndex>>,
    pub deposit_whitelist: UnorderedSet<AccountId>,
}

#[near_bindgen]
impl Ownable for Contract {
    fn get_owner(&self) -> AccountId {
        self.owner_id.clone()
    }

    fn set_owner(&mut self, owner: AccountId) {
        self.assert_owner();
        self.owner_id = owner;
    }
}

#[near_bindgen]
impl Contract {
    /// Migration function for contract upgrade
    #[init(ignore_state)]
    #[private]
    pub fn migrate(owner_id: Option<AccountId>) -> Self {
        let contract: OldContract = env::state_read().unwrap_or_else(|| panic!("Not initialized"));

        Self {
            token_account_id: contract.token_account_id,
            lockups: contract.lockups,
            account_lockups: contract.account_lockups,
            deposit_whitelist: contract.deposit_whitelist,
            blacklist: UnorderedSet::new(StorageKey::Blacklist),
            owner_id: owner_id.unwrap_or_else(|| env::predecessor_account_id()),
        }
    }
}
