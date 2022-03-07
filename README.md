# Fungible Token Lockup contract

## Features

- A reusable lockup contract for a select fungible token.
- Lockup schedule can be set as a list of checkpoints with time and balance.
- Supports multiple lockups per account ID.
- Ability to create a lockup that can be terminated
  - A single lockup can be only terminated by a specific account ID.
  - Supports custom vesting schedule that should be ahead of the lockup schedule
  - The vesting schedule can be hidden behind a hash, so it only needs to be revealed in case of termnation.
- Automatic rollbacks if a FT transfer fails.
- Claiming all account's lockups in a single transaction.
- Ability to add new lockups.
- Whitelist for the accounts that can create new lockups.


# Usage

### setup variables
```shell
export OWNER_ID=owner_account.testnet
export USER_ID=user_account.testnet
export TOKEN_CONTRACT_ID=wrap.testnet
export LOCKUP_CONTRACT_ID=lockup.$OWNER_ID
```

### create subaccount for lockup contract
```shell
near create-account $LOCKUP_CONTRACT_ID --masterAccount $OWNER_ID
```

### deploy and initialize
```shell
near deploy --accountId $LOCKUP_CONTRACT_ID --wasmFile ft_lockup.wasm --initFunction new --initArgs '{"token_account_id": "'$TOKEN_CONTRACT_ID'", "deposit_whitelist": ["'$OWNER_ID'"]}'
```

### register lockup contract in token contract
```shell
near call $TOKEN_CONTRACT_ID storage_deposit '{"account_id": "'$LOCKUP_CONTRACT_ID'"}' --accountId $OWNER_ID --amount .00125
```

### add 1 wNEAR (24 decimals) with linear lockup for one year
```shell  
TIMESTAMP=$(date +%s)
ONE_YEAR_LATER=$((TIMESTAMP+365*24*60*60))
AMOUNT=1000000000000000000000000 
ONE_YEAR_LINEAR_LOCKUP='{"account_id":"'$USER_ID'","schedule":[{"timestamp":'$TIMESTAMP',"balance":"0"},{"timestamp":'$ONE_YEAR_LATER',"balance":"'$AMOUNT'"}],"claimed_balance":"0"}'
ONE_YEAR_LINEAR_LOCKUP_ESC=$(echo $ONE_YEAR_LINEAR_LOCKUP | perl -pe 's/\"/\\"/g')

near call $TOKEN_CONTRACT_ID ft_transfer_call '{"receiver_id": "'$LOCKUP_CONTRACT_ID'","amount": "'$AMOUNT'","msg":"'$ONE_YEAR_LINEAR_LOCKUP_ESC'"}' --account-id $OWNER_ID --gas 300000000000000 --amount .000000000000000000000001
```

### check user lockups
```shell
near view $LOCKUP_CONTRACT_ID get_account_lockups '{"account_id": "'$USER_ID'"}'
```

### check token balance
```shell
near view $TOKEN_CONTRACT_ID ft_balance_of '{"account_id": "'$USER_ID'"}'
```

### claim all user lockups (should require 1 yocto?)
```shell
near call $LOCKUP_CONTRACT_ID claim '' --account-id $USER_ID --gas 300000000000000
```
