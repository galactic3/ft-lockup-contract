#!/bin/bash

rm -rf neardev
rm -f /tmp/empty.wasm
touch /tmp/empty.wasm
near dev-deploy --wasmFile /tmp/empty.wasm
rm /tmp/empty.wasm

ACC_ROOT=$(cat neardev/dev-account)
ACC_OWNER=owner.$ACC_ROOT
ACC_TOKEN=token.$ACC_ROOT
ACC_LOCKUP=ft-lockup.$ACC_ROOT
ACC_ALICE=alice.$ACC_ROOT

echo "ACC_ROOT=$ACC_ROOT"

near create-account --masterAccount $ACC_ROOT --initialBalance 10.0 $ACC_OWNER
near create-account --masterAccount $ACC_ROOT --initialBalance 10.0 $ACC_TOKEN
near create-account --masterAccount $ACC_ROOT --initialBalance 10.0 $ACC_ALICE

FT_SUPPLY=1000000
FT_DECIMALS=0
FT_LOCKUP_AMOUNT=60000

near deploy --accountId $ACC_TOKEN --wasmFile res/fungible_token.wasm --initFunction new --initArgs '
{
  "owner_id": "'$ACC_OWNER'",
  "total_supply": "'$FT_SUPPLY'",
  "metadata": {
    "spec": "ft-1.0.0",
    "name": "Token",
    "symbol": "TOKEN",
    "icon": null,
    "reference": null,
    "reference_hash": null,
    "decimals": '$FT_DECIMALS'
  }
}
'

near create-account --masterAccount $ACC_ROOT --initialBalance 10.0 $ACC_LOCKUP

near deploy --accountId $ACC_LOCKUP --wasmFile res/ft_lockup.wasm --initFunction new --initArgs '
{
  "token_account_id": "'$ACC_TOKEN'",
  "deposit_whitelist": [
    "'$ACC_OWNER'"
  ]
}
'

near call --accountId $ACC_OWNER $ACC_TOKEN storage_deposit --deposit 0.125 '
{
  "account_id": "'$ACC_LOCKUP'"
}
'

near call --accountId $ACC_OWNER $ACC_TOKEN storage_deposit --deposit 0.125 '
{
  "account_id": "'$ACC_ALICE'"
}
'

near call --accountId $ACC_OWNER $ACC_TOKEN ft_balance_of '
{
  "account_id": "'$ACC_OWNER'"
}
'

near call --accountId $ACC_OWNER $ACC_TOKEN ft_balance_of '
{
  "account_id": "'$ACC_LOCKUP'"
}
'

near call --accountId $ACC_OWNER $ACC_TOKEN ft_balance_of '
{
  "account_id": "'$ACC_ALICE'"
}
'

echo 'BASIC SETUP FINISHED, CREATING LOCKUP'

# vesting is same as schedule
LOCKUP_START_TIMESTAMP=1700000000
LOCKUP_FINISH_TIMESTAMP=1800000000

LOCKUP_MESSAGE='
{
  "account_id": "'$ACC_ALICE'",
  "schedule": [
    {
      "timestamp": '$((LOCKUP_START_TIMESTAMP))',
      "balance": "0"
    },
    {
      "timestamp": '$((LOCKUP_FINISH_TIMESTAMP))',
      "balance": "'$FT_LOCKUP_AMOUNT'"
    }
  ],
  "claimed_balance": "0",
  "termination_config": {
    "terminator_id": "'$ACC_OWNER'",
    "vesting_schedule": null
  }
}
'

echo $LOCKUP_MESSAGE

LOCKUP_MESSAGE_ESCAPED=$(echo $LOCKUP_MESSAGE | sed -e 's/"/\\"/g')

ONE_YOCTO=0.000000000000000000000001
MAX_GAS=300000000000000

near call --accountId $ACC_OWNER $ACC_TOKEN ft_transfer_call --gas $MAX_GAS --deposit $ONE_YOCTO '
{
  "receiver_id": "'$ACC_LOCKUP'",
  "amount": "'$FT_LOCKUP_AMOUNT'",
  "msg": "'"$LOCKUP_MESSAGE_ESCAPED"'"
}
'

echo "LOCKUP CREATED"
near view $ACC_LOCKUP get_account_lockups '{ "account_id": "'$ACC_ALICE'" }'

echo 'TERMINATING'
near call --accountId $ACC_OWNER $ACC_LOCKUP terminate --gas $MAX_GAS '{ "lockup_index": 0 }'
echo 'OWNER FT BALANCE'
near call --accountId $ACC_OWNER $ACC_TOKEN ft_balance_of '{ "account_id": "'$ACC_OWNER'" }'

echo 'CLAIMING AFTER TERMINATE'
near call --accountId $ACC_ALICE $ACC_LOCKUP claim --gas $MAX_GAS '{ }'
echo 'CLAIMED FT BALANCE'
near call --accountId $ACC_OWNER $ACC_TOKEN ft_balance_of '{ "account_id": "'$ACC_ALICE'" }'

near view $ACC_LOCKUP get_account_lockups '{ "account_id": "'$ACC_ALICE'" }'

echo 'LOCKUP IS NOT REMOVED AFTER CLAIM, since unclaimed_amount = 0'

echo 'CREATING ANOTHER UNLOCKED LOCKUP, to allow previous lockup to be removed'

# vesting is same as schedule
FT_LOCKUP_AUX_AMOUNT=1

LOCKUP_AUX_MESSAGE='
{
  "account_id": "'$ACC_ALICE'",
  "schedule": [
    { "timestamp": 0, "balance": "0" },
    { "timestamp": 1, "balance": "1" }
  ],
  "claimed_balance": "0",
  "termination_config": null
}
'

echo $LOCKUP_MESSAGE

LOCKUP_AUX_MESSAGE_ESCAPED=$(echo $LOCKUP_AUX_MESSAGE | sed -e 's/"/\\"/g')

ONE_YOCTO=0.000000000000000000000001
MAX_GAS=300000000000000

near call --accountId $ACC_OWNER $ACC_TOKEN ft_transfer_call --gas $MAX_GAS --deposit $ONE_YOCTO '
{
  "receiver_id": "'$ACC_LOCKUP'",
  "amount": "'$FT_LOCKUP_AUX_AMOUNT'",
  "msg": "'"$LOCKUP_AUX_MESSAGE_ESCAPED"'"
}
'

echo 'CHECKING LOCKUPS'
near view $ACC_LOCKUP get_account_lockups '{ "account_id": "'$ACC_ALICE'" }'

echo 'CLAIMING WITH AUX LOCKUP'
near call --accountId $ACC_ALICE $ACC_LOCKUP claim --gas $MAX_GAS '{ }'

echo 'FINAL CLAIMED FT BALANCE'
near call --accountId $ACC_OWNER $ACC_TOKEN ft_balance_of '{ "account_id": "'$ACC_ALICE'" }'

echo 'FINAL OWNER FT BALANCE'
near call --accountId $ACC_OWNER $ACC_TOKEN ft_balance_of '{ "account_id": "'$ACC_ALICE'" }'

echo 'CHECKING LOCKUPS AFTER CLAIM WITH AUX'
near view $ACC_LOCKUP get_account_lockups '{ "account_id": "'$ACC_ALICE'" }'

echo 'ZERO AMOUNT LOCKUP IS NOT REMOVED'

