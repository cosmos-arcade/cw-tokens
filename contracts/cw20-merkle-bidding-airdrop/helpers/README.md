merkle-airdrop-cli
==================

This is a helper client shipped along contract.
Use this to generate root, generate proofs and verify proofs

## Installation

```shell
yarn install
yarn link
```

Binary will be placed to path.

## Airdrop file format

```json
[
  { "address": "wasm1k9hwzxs889jpvd7env8z49gad3a3633vg350tq", "amount": "100"},
  { "address": "wasm1uy9ucvgerneekxpnfwyfnpxvlsx5dzdpf0mzjd", "amount": "1010"}
]
```

## Commands

**Generate Root:**
```shell
merkle-airdrop-cli generateRoot --file ../testdata/airdrop_game_list.json

merkle-airdrop-cli generateRoot --file ../testdata/airdrop_list.json
```

**Generate proof:**
```shell
merkle-airdrop-cli generateProofs --file ../testdata/airdrop_game_list.json \
  --address wasm1a4x6au55s0fusctyj2ulrxvfpmjcxa92k7ze2v \
  --amount 10

merkle-airdrop-cli generateProofs --file ../testdata/airdrop_list.json \
  --address wasm1a4x6au55s0fusctyj2ulrxvfpmjcxa92k7ze2v \
  --amount 10220
```

**Verify proof:**
```shell
PROOFS='["a714186eaedddde26b08b9afda38cf62fdf88d68e3aa0d5a4b55033487fe14a1","fb57090a813128eeb953a4210dd64ee73d2632b8158231effe2f0a18b2d3b5dd","c30992d264c74c58b636a31098c6c27a5fc08b3f61b7eafe2a33dcb445822343"]'
merkle-airdrop-cli verifyProofs --file ../testdata/airdrop_list.json \
  --address wasm1k9hwzxs889jpvd7env8z49gad3a3633vg350tq \
  --amount 100 \
  --proofs $PROOFS
```
