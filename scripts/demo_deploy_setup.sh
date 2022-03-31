#!/bin/bash

# rm -rf neardev
# rm -f /tmp/empty.wasm
# touch /tmp/empty.wasm
# near dev-deploy --wasmFile /tmp/empty.wasm
# rm /tmp/empty.wasm

# ACC_ROOT=$(cat neardev/dev-account)

ACC_ROOT='demo000.ft-lockup.testnet'

ACC_OWNER=owner.$ACC_ROOT
ACC_TOKEN=token.$ACC_ROOT
ACC_LOCKUP=ft-lockup.$ACC_ROOT
ACC_ALICE=alice.$ACC_ROOT
ACC_BOB=bob.$ACC_ROOT
ACC_CAROL=carol.$ACC_ROOT

echo "ACC_ROOT=$ACC_ROOT"

near create-account --masterAccount $ACC_ROOT --initialBalance 10.0 $ACC_OWNER
near create-account --masterAccount $ACC_ROOT --initialBalance 10.0 $ACC_TOKEN
near create-account --masterAccount $ACC_ROOT --initialBalance 10.0 $ACC_ALICE
near create-account --masterAccount $ACC_ROOT --initialBalance 10.0 $ACC_BOB
near create-account --masterAccount $ACC_ROOT --initialBalance 10.0 $ACC_CAROL

FT_SUPPLY=1000000000000000000
FT_DECIMALS=6
FT_LOCKUP_AMOUNT=100000000000

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

VESTING_TIMESTAMPS=(1500000000 1549999999 1550000000 1700000000)
VESTING_AMOUNTS=(0 10 25 100)

LOCKUP_TIMESTAMPS=(1500000000 1600000000 1700000000)
LOCKUP_AMOUNTS=(0 0 100)

LOCKUP_MESSAGE='
{
  "account_id": "'$ACC_ALICE'",
  "schedule": [
    { "timestamp": '${LOCKUP_TIMESTAMPS[0]}', "balance": "'${LOCKUP_AMOUNTS[0]}000000000'" },
    { "timestamp": '${LOCKUP_TIMESTAMPS[1]}', "balance": "'${LOCKUP_AMOUNTS[1]}000000000'" },
    { "timestamp": '${LOCKUP_TIMESTAMPS[2]}', "balance": "'${LOCKUP_AMOUNTS[2]}000000000'" }
  ],
  "claimed_balance": "0",
  "termination_config": {
    "terminator_id": "'$ACC_OWNER'",
    "vesting_schedule": {
      "Schedule": [
        { "timestamp": '${VESTING_TIMESTAMPS[0]}', "balance": "'${VESTING_AMOUNTS[0]}000000000'" },
        { "timestamp": '${VESTING_TIMESTAMPS[1]}', "balance": "'${VESTING_AMOUNTS[1]}000000000'" },
        { "timestamp": '${VESTING_TIMESTAMPS[2]}', "balance": "'${VESTING_AMOUNTS[2]}000000000'" },
        { "timestamp": '${VESTING_TIMESTAMPS[3]}', "balance": "'${VESTING_AMOUNTS[3]}000000000'" }
      ]
    }
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

near view $ACC_LOCKUP get_account_lockups '
{
  "account_id": "'$ACC_ALICE'"
}
'

echo 'CREATING LOCKUP 2 FULLY IN THE PAST'

LOCKUP_TIMESTAMPS=(1500000000 1600000000)
LOCKUP_AMOUNTS=(0 100)

LOCKUP_START_TIMESTAMP=1700000000
LOCKUP_FINISH_TIMESTAMP=1800000000

LOCKUP_MESSAGE='
{
  "account_id": "'$ACC_ALICE'",
  "schedule": [
    { "timestamp": '${LOCKUP_TIMESTAMPS[0]}', "balance": "'${LOCKUP_AMOUNTS[0]}000000000'" },
    { "timestamp": '${LOCKUP_TIMESTAMPS[1]}', "balance": "'${LOCKUP_AMOUNTS[1]}000000000'" }
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

echo "LOCKUP 2 CREATED"
echo 'CREATING LOCKUP 3 FULLY IN THE FUTURE'

LOCKUP_TIMESTAMPS=(1700000000 1800000000)
LOCKUP_AMOUNTS=(0 100)

LOCKUP_MESSAGE='
{
  "account_id": "'$ACC_ALICE'",
  "schedule": [
    { "timestamp": '${LOCKUP_TIMESTAMPS[0]}', "balance": "'${LOCKUP_AMOUNTS[0]}000000000'" },
    { "timestamp": '${LOCKUP_TIMESTAMPS[1]}', "balance": "'${LOCKUP_AMOUNTS[1]}000000000'" }
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

echo "LOCKUP 3 CREATED"
echo 'CREATING LOCKUP FOR BOB'

LOCKUP_TIMESTAMPS=(1600000000 1700000000)
LOCKUP_AMOUNTS=(0 100)

LOCKUP_MESSAGE='
{
  "account_id": "'$ACC_BOB'",
  "schedule": [
    { "timestamp": '${LOCKUP_TIMESTAMPS[0]}', "balance": "'${LOCKUP_AMOUNTS[0]}000000000'" },
    { "timestamp": '${LOCKUP_TIMESTAMPS[1]}', "balance": "'${LOCKUP_AMOUNTS[1]}000000000'" }
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

echo "LOCKUP FOR BOB CREATED"
echo 'CREATING LOCKUP FOR CAROL'

LOCKUP_TIMESTAMPS=(1600000000 1700000000)
LOCKUP_AMOUNTS=(0 100)

LOCKUP_MESSAGE='
{
  "account_id": "'$ACC_CAROL'",
  "schedule": [
    { "timestamp": '${LOCKUP_TIMESTAMPS[0]}', "balance": "'${LOCKUP_AMOUNTS[0]}000000000'" },
    { "timestamp": '${LOCKUP_TIMESTAMPS[1]}', "balance": "'${LOCKUP_AMOUNTS[1]}000000000'" }
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

echo "LOCKUP FOR CAROL CREATED"

echo 'CLAIMING'
near call --accountId $ACC_ALICE $ACC_LOCKUP claim --gas $MAX_GAS '{ }'
echo 'CLAIMED FT BALANCE'
near call --accountId $ACC_OWNER $ACC_TOKEN ft_balance_of '{ "account_id": "'$ACC_ALICE'" }'

echo 'CHECKING ALL LOCKUPS'
near view $ACC_LOCKUP get_lockups_paged '{}'

echo 'FINISH'
