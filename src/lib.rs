use std::collections::HashMap;
use std::convert::TryInto;

use near_contract_standards::fungible_token::core_impl::ext_fungible_token;
use near_contract_standards::fungible_token::receiver::FungibleTokenReceiver;
use near_sdk::borsh::maybestd::collections::HashSet;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LookupMap, UnorderedSet, Vector};
use near_sdk::json_types::{Base58CryptoHash, ValidAccountId, WrappedBalance, U128};
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{
    assert_one_yocto, env, ext_contract, is_promise_success, log, near_bindgen, serde_json,
    AccountId, Balance, BorshStorageKey, CryptoHash, Gas, PanicOnDefault, PromiseOrValue,
    Timestamp,
};

pub mod callbacks;
pub mod draft;
pub mod ft_token_receiver;
pub mod internal;
pub mod lockup;
pub mod schedule;
pub mod termination;
pub mod util;
pub mod view;

use crate::draft::*;
use crate::lockup::*;
use crate::schedule::*;
use crate::termination::*;
use crate::util::*;

near_sdk::setup_alloc!();

pub type TimestampSec = u32;
pub type TokenAccountId = AccountId;

const GAS_FOR_FT_TRANSFER: Gas = 15_000_000_000_000;
const GAS_FOR_AFTER_FT_TRANSFER: Gas = 20_000_000_000_000;

const ONE_YOCTO: Balance = 1;
const NO_DEPOSIT: Balance = 0;

uint::construct_uint! {
    pub struct U256(4);
}

#[ext_contract(ext_self)]
pub trait SelfCallbacks {
    fn after_ft_transfer(
        &mut self,
        account_id: AccountId,
        lockup_claims: Vec<LockupClaim>,
    ) -> WrappedBalance;

    fn after_lockup_termination(
        &mut self,
        account_id: AccountId,
        amount: WrappedBalance,
    ) -> WrappedBalance;
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct Contract {
    pub token_account_id: TokenAccountId,

    pub lockups: Vector<Lockup>,

    pub account_lockups: LookupMap<AccountId, HashSet<LockupIndex>>,

    /// Account IDs that can create new lockups.
    pub deposit_whitelist: UnorderedSet<AccountId>,

    pub next_draft_id: DraftIndex,
    pub drafts: LookupMap<DraftIndex, Draft>,
    pub draft_groups: Vector<DraftGroup>,
}

#[derive(BorshStorageKey, BorshSerialize)]
pub(crate) enum StorageKey {
    Lockups,
    AccountLockups,
    DepositWhitelist,
    Drafts,
    DraftGroups,
}

#[near_bindgen]
impl Contract {
    #[init]
    pub fn new(token_account_id: ValidAccountId, deposit_whitelist: Vec<ValidAccountId>) -> Self {
        let mut deposit_whitelist_set = UnorderedSet::new(StorageKey::DepositWhitelist);
        deposit_whitelist_set.extend(deposit_whitelist.into_iter().map(|a| a.into()));
        Self {
            lockups: Vector::new(StorageKey::Lockups),
            account_lockups: LookupMap::new(StorageKey::AccountLockups),
            token_account_id: token_account_id.into(),
            deposit_whitelist: deposit_whitelist_set,
            next_draft_id: 0,
            drafts: LookupMap::new(StorageKey::Drafts),
            draft_groups: Vector::new(StorageKey::DraftGroups),
        }
    }

    pub fn claim_lockups(
        &mut self,
        amounts: Vec<(LockupIndex, WrappedBalance)>,
    ) -> PromiseOrValue<WrappedBalance> {
        let amounts: HashMap<LockupIndex, WrappedBalance> = amounts.into_iter().collect();
        let account_id = env::predecessor_account_id();
        let lockups_by_id: HashMap<LockupIndex, Lockup> = self
            .internal_get_account_lockups_by_id(&account_id, &amounts.keys().map(|&x| x).collect())
            .into_iter()
            .collect();
        self.internal_claim_lockups(amounts, lockups_by_id)
    }

    pub fn claim(&mut self) -> PromiseOrValue<WrappedBalance> {
        let account_id = env::predecessor_account_id();
        let lockups_by_id: HashMap<LockupIndex, Lockup> = self
            .internal_get_account_lockups(&account_id)
            .into_iter()
            .collect();
        let amounts: HashMap<LockupIndex, WrappedBalance> = lockups_by_id
            .iter()
            .map(|(lockup_id, lockup)| {
                let unlocked_balance = lockup.schedule.unlocked_balance(current_timestamp_sec());
                let amount: WrappedBalance = (unlocked_balance - lockup.claimed_balance).into();

                (lockup_id.clone(), amount)
            })
            .collect();

        self.internal_claim_lockups(amounts, lockups_by_id)
    }

    pub fn terminate(
        &mut self,
        lockup_index: LockupIndex,
        hashed_schedule: Option<Schedule>,
        termination_timestamp: Option<TimestampSec>,
    ) -> PromiseOrValue<WrappedBalance> {
        self.assert_deposit_whitelist(&env::predecessor_account_id());
        let mut lockup = self
            .lockups
            .get(lockup_index as _)
            .expect("Lockup not found");
        let current_timestamp = current_timestamp_sec();
        let termination_timestamp = termination_timestamp.unwrap_or(current_timestamp);
        assert!(
            termination_timestamp >= current_timestamp,
            "expected termination_timestamp >= now",
        );
        let (unvested_balance, payer_id) =
            lockup.terminate(hashed_schedule, termination_timestamp);
        self.lockups.replace(lockup_index as _, &lockup);

        // no need to store empty lockup
        if lockup.schedule.total_balance() == 0 {
            let lockup_account_id: AccountId = lockup.account_id.into();
            let mut indices = self
                .account_lockups
                .get(&lockup_account_id)
                .unwrap_or_default();
            indices.remove(&lockup_index);
            self.internal_save_account_lockups(&lockup_account_id, indices);
        }

        if unvested_balance > 0 {
            ext_fungible_token::ft_transfer(
                payer_id.clone().into(),
                unvested_balance.into(),
                Some(format!("Terminated lockup #{}", lockup_index)),
                &self.token_account_id,
                ONE_YOCTO,
                GAS_FOR_FT_TRANSFER,
            )
            .then(ext_self::after_lockup_termination(
                payer_id.clone().into(),
                unvested_balance.into(),
                &env::current_account_id(),
                NO_DEPOSIT,
                GAS_FOR_AFTER_FT_TRANSFER,
            ))
            .into()
        } else {
            PromiseOrValue::Value(0.into())
        }
    }

    #[payable]
    pub fn add_to_deposit_whitelist(&mut self, account_id: ValidAccountId) {
        assert_one_yocto();
        self.assert_deposit_whitelist(&env::predecessor_account_id());
        self.deposit_whitelist.insert(account_id.as_ref());
    }

    #[payable]
    pub fn remove_from_deposit_whitelist(&mut self, account_id: ValidAccountId) {
        assert_one_yocto();
        self.assert_deposit_whitelist(&env::predecessor_account_id());
        self.deposit_whitelist.remove(account_id.as_ref());
    }

    pub fn create_draft_group(&mut self) -> DraftGroupIndex {
        self.assert_deposit_whitelist(&env::predecessor_account_id());

        let index: DraftGroupIndex = self.draft_groups.len() as _;
        self.draft_groups.push(&DraftGroup::new());

        index
    }

    pub fn create_draft(&mut self, draft: Draft) -> DraftIndex {
        self.assert_deposit_whitelist(&env::predecessor_account_id());
        draft.assert_new_valid(&env::predecessor_account_id().try_into().unwrap());
        let mut draft_group = self
            .draft_groups
            .get(draft.draft_group_id as _)
            .expect("draft group not found");
        draft_group.assert_can_add_draft();

        let index = self.next_draft_id;
        self.next_draft_id += 1;
        assert!(self.drafts.insert(&index, &draft).is_none(), "Invariant");

        draft_group.total_amount += draft.total_balance();
        draft_group.draft_indices.insert(index);
        self.draft_groups
            .replace(draft.draft_group_id as _, &draft_group);

        index
    }

    pub fn create_drafts(&mut self, drafts: Vec<Draft>) -> Vec<DraftIndex> {
        drafts
            .into_iter()
            .map(|draft| self.create_draft(draft))
            .collect()
    }

    pub fn convert_draft(&mut self, draft_id: DraftIndex) -> LockupIndex {
        let mut draft = self.drafts.remove(&draft_id).expect("draft not found");
        let mut draft_group = self
            .draft_groups
            .get(draft.draft_group_id as _)
            .expect("draft group not found");
        draft_group.assert_can_convert();

        // remove draft from indices and total amount
        assert!(draft_group.draft_indices.remove(&draft_id), "Invariant");
        let amount = draft.total_balance();
        assert!(draft_group.total_amount >= amount, "Invariant");
        draft_group.total_amount -= amount;
        self.draft_groups
            .replace(draft.draft_group_id as _, &draft_group);

        // update payer_id
        if let Some(mut termination_config) = draft.lockup.termination_config {
            termination_config.payer_id = draft_group.payer_id.unwrap();
            draft.lockup.termination_config = Some(termination_config);
        }
        let index = self.internal_add_lockup(&draft.lockup);
        log!(
            "Created new lockup for {} with index {} from draft {}",
            draft.lockup.account_id.as_ref(),
            index,
            draft_id,
        );

        index
    }

    pub fn convert_drafts(&mut self, draft_ids: Vec<DraftIndex>) -> Vec<LockupIndex> {
        draft_ids
            .into_iter()
            .map(|draft_id| self.convert_draft(draft_id))
            .collect()
    }
}
