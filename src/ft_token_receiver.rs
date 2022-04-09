use crate::*;

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct DraftGroupConfirmation {
    pub draft_group_id: DraftGroupIndex,
}

#[near_bindgen]
impl FungibleTokenReceiver for Contract {
    fn ft_on_transfer(
        &mut self,
        sender_id: ValidAccountId,
        amount: U128,
        msg: String,
    ) -> PromiseOrValue<U128> {
        assert_eq!(
            env::predecessor_account_id(),
            self.token_account_id,
            "Invalid token ID"
        );
        let amount = amount.into();
        self.assert_deposit_whitelist(sender_id.as_ref());
        let lockup: Result<Lockup, _> = serde_json::from_str(&msg);
        if let Ok(lockup) = lockup {
            lockup.assert_new_valid(amount);
            let index = self.internal_add_lockup(&lockup);
            log!(
                "Created new lockup for {} with index {}",
                lockup.account_id.as_ref(),
                index
            );
            return PromiseOrValue::Value(0.into())
        }
        let confirmation: Result<DraftGroupConfirmation, _> = serde_json::from_str(&msg);
        if let Ok(confirmation) = confirmation {
            let draft_group_id = confirmation.draft_group_id;
            let mut draft_group = self
                .draft_groups
                .get(draft_group_id as _)
                .expect("draft group not found");
            assert_eq!(
                draft_group.total_amount,
                amount,
                "The draft group total balance doesn't match the transferred balance",
            );
            // panic!("mark0");
            assert!(!draft_group.funded, "draft group already funded");
            draft_group.funded = true;
            self.draft_groups.replace(draft_group_id as _, &draft_group);
            log!("Funded draft group {}", draft_group_id);
            return PromiseOrValue::Value(0.into())
        }

        panic!("Expected Lockup or DraftGroupConfirmation as msg");
    }
}
