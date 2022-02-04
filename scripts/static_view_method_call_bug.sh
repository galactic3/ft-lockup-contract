#!/bin/bash

rm -rf neardev
near dev-deploy --wasmFile res/ft_lockup.wasm --initFunction new --initArgs '
{
  "token_account_id": "'sometoken.testnet'",
  "deposit_whitelist": [
    "'somebody.testnet'"
  ]
}
'

ACC_ROOT=$(cat neardev/dev-account)

near view $ACC_ROOT hash_schedule '
{
  "schedule": []
}
'

echo "METHOD FAILS INSIDE SDK INERNALS"
