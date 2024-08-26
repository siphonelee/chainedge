### Private key
Ensure you create a .env file in the directory. Then paste your Metamask private key in .env with the variable name ACCOUNT_PRIVATE_KEY as follows:
ACCOUNT_PRIVATE_KEY=0x123...

### Compile
forge compile

### Test
forge test

### Deploy
forge script script/DeployChainEdge.s.sol --broadcast --rpc-url https://rpc.open-campus-codex.gelato.digital/ --gas-limit 30000000 --with-gas-price 5gwei --skip-simulation

### Verify
forge verify-contract \
  --rpc-url https://rpc.open-campus-codex.gelato.digital \
  --verifier blockscout \
  --verifier-url 'https://opencampus-codex.blockscout.com/api/' \
  <deployed-contract-address> \
  src/ChainEdge.sol:ChainEdge

### Generate ABI JSON
forge build --silent && jq '.abi' ./out/ChainEdge.sol/ChainEdge.json

### Call
cast send 0x365D9FFd3334d12f89f8510cd352b3DbB5f4Cf85 "addToCDN(string[])" "[get	/url1/x?a=b, post	/url2]" --rpc-url https://rpc.open-campus-codex.gelato.digital/ --private-key $ACCOUNT_PRIVATE_KEY
cast send 0x365D9FFd3334d12f89f8510cd352b3DbB5f4Cf85 "removeFromCDN(string[])" "[get	/url1/x?a=b,post	/url2]" --rpc-url https://rpc.open-campus-codex.gelato.digital/ --private-key $ACCOUNT_PRIVATE_KEY
cast call 0x365D9FFd3334d12f89f8510cd352b3DbB5f4Cf85 "getCDNList()(string[])" --rpc-url https://rpc.open-campus-codex.gelato.digital/ --private-key $ACCOUNT_PRIVATE_KEY

cast send 0x365D9FFd3334d12f89f8510cd352b3DbB5f4Cf85 "addToCDN(string[])" "[get	/slow, get	/fast]" --rpc-url https://rpc.open-campus-codex.gelato.digital/ --private-key $ACCOUNT_PRIVATE_KEY
