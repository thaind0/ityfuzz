#ItyFuzz 🍦

ItyFuzz is a fast hybrid fuzz testing tool for EVM, MoveVM (WIP) and more.

Just provide the contract address and the vulnerability will be found **immediately**:
![](https://ityfuzz.assets.fuzz.land/demo2.gif)

[English version README](https://github.com/fuzzland/ityfuzz/blob/master/README.md) / [Research Paper](https://scf.so/ityfuzz.pdf) / [Development Information](#development)

# Statistics

Time taken to discover vulnerability/generate attack:

| Project Name  | Vulnerability      | **Mythril** | **SMARTIAN**  | **Slither** | **ItyFuzz** |
| ------------- | ------------------ | ----------- | ------------- | ----------- | ----------- |
| AES           | Business Logic     | Inf         | Not Supported | No          | 4 Hours     |
| Carrot        | Any external call  | 17s         | 11s           | Yes         | 1s          |
| Olympus       | Access Control     | 36s         | Inf           | Yes         | 1s          |
| MUMUG         | Price Manipulation | Inf         | Not Supported | No          | 18 Hours    |
| Omni          | Reentrancy         | Inf         | Not supported | Yes\*       | 22 hours    |
| Verilog CTF-2 | Reentrancy         | Inf         | Not supported | Yes\*       | 3s          |

<sub>\* Slither only discovers the location of reentrancy, not how the reentrancy is exploited to trigger the final error code. The output also contains a large number of false positives. </sub>

Test coverage:

| **Dataset** | **SMARTIAN**  | **Echidna** | **ItyFuzz** |
| ----------- | ------------- | ----------- | ----------- |
| B1          | 97.1%         | 47.1%       | 99.2%       |
| B2          | 86.2%         | 82.9%       | 95.4%       |
| Tests       | Not supported | 52.9%       | 100%        |

<sub>\* B1 and B2 contain 72 contracts. Tests are projects in the `tests` directory. Coverage is calculated as `(Instructions Covered)/(Total Instructions - Invalid Code)`. </sub>

# Install

## 1. ityfuzzup (recommended)

```bash
curl -L https://raw.githubusercontent.com/fuzzland/ityfuzz/master/ityfuzzup/install | bash
```

## 2. Release

Download the latest [release](https://github.com/fuzzland/ityfuzz/releases/latest)

## 3. Docker

Install [Docker](https://www.docker.com/) and run the docker image appropriate for your system architecture:

```
docker pull fuzzland/ityfuzz:stable
docker run -p 8000:8000 fuzzland/ityfuzz:stable
```

You can then access the UI at http://localhost:8000.

<sub>Note: Containers use public ETH RPC and may timeout or run slowly</sub>

## 4. Build from source

You need to install `libssl-dev` (OpenSSL) and `libz3-dev` (see the instructions in the [Z3 Installation](#z3-installation) section).

```bash
# Download dependencies
git submodule update --recursive --init
cargo build --release
```

You need `solc` to compile smart contracts. You can use the `solc-select` tool to manage `solc` versions.

# run

Compile the smart contract:

```bash
cd ./tests/multi-contract/
solc *.sol -o . --bin --abi --overwrite --base-path ../../
```

Run the fuzzer:

```bash
./cli -t '../tests/multi-contract/*'
```

### Demo

**Verilog CTF Challenge 2**
`tests/verilog-2/`

The contract has flash loan attack + re-entrancy vulnerability. The attack targets line 34 in `Bounty.sol`.

Specific vulnerability exploitation process:

```
0. Borrow k MATIC so that k > balance() / 10
1. Call depositMATIC() with k MATIC
2. redeem(k * 1e18) --re-enter the contract --> getBounty()
3. Return k MATIC
```

Use ItyFuzz to detect vulnerabilities and generate specific exploits (takes 0-200 seconds):

```bash
# Build the contract in tests/verilog-2/
solc *.sol -o . --bin --abi --overwrite --base-path ../../
# Run the fuzzer
ityfuzz evm -f -t "../tests/evm/verilog-2/*"
```

The `-f` flag enables automatic flash lending, which hooks all ERC20 external calls, allowing any user to have an unlimited balance.

### Offline Fuzz a project

You can fuzz a project by providing the path (glob) to the project directory.

```bash
ityfuzz evm -t '[DIR_PATH]/*'
```

ItyFuzz will attempt to deploy all artifacts in the directory to the blockchain without other smart contracts.
The project directory should contain `[X].abi` and `[X].bin` files. For example, to fuzz a contract named `main.sol`, you would
Make sure `main.abi` and `main.bin` exist in the project directory.
ItyFuzz will automatically detect associations between contracts in the directory (see `tests/multi-contract`),
and fuzz them.

If ItyFuzz is unable to infer the correlation between contracts, you
You can also add a `[X].address`, where `[X]` is the contract name, to specify the address of the contract.

Precautions:

- ItyFuzz performs fuzz on the blockchain without any contracts,
  Therefore you should ensure that all relevant contracts (e.g. ERC20 tokens, Uniswap, etc.) will be deployed to ItyFuzz's blockchain before fuzzing.

### Online Fuzz a project

Ityfuzz will first read the `ETH_RPC_URL` environment variable as the RPC address, if not set, the built-in public RPC address will be used.

You can fuzz a project by providing the address, block and chain.

```bash
ityfuzz evm -o -t [TARGET_ADDR] --onchain-block-number [BLOCK] -c [CHAIN_TYPE] --onchain-etherscan-api-key [Etherscan API Key]
```

Example:
fuzz WETH contract (`0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2`) on the latest block on the Ethereum mainnet.

```bash
ityfuzz evm -o -t 0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2 --onchain-block-number 0 -c ETH --onchain-etherscan-api-key PXUUKVEQ7Y4VCQYPQC2CEK4CAKF8SG7MVF
```

ItyFuzz will pull the contract's ABI from Etherscan and fuzz it. If ItyFuzz encounters an unknown slot in Storage, it will synchronize the slot from RPC.
If ItyFuzz encounters a call to an external unknown contract, it will pull the bytecode and ABI of that contract. If its ABI is not available, ItyFuzz will use heimdall to decompile the bytecode to analyze the ABI.

### Onchain Get

When encountering a SLOAD with an uninitialized target, ItyFuzz attempts to obtain storage from the blockchain node. There are three ways to obtain it:

- OneByOne: Get one slot at a time. This is the default mode. It's slow, but it doesn't fail.
- Dump: Use debug API `debug_storageRangeAt` to dump storage. This only works with ETH (for now) and can easily fail.

### Constructor parameters

ItyFuzz provides two methods to pass in constructor parameters. These parameters are necessary to initialize the contract's state upon deployment.

**Method 1: CLI Parameters**

The first method is to pass the constructor parameters directly as CLI parameters.

When you use the CLI

When running ItyFuzz, you can include the `--constructor-args` flag, followed by a string specifying the arguments for each constructor.

The format is as follows:

```
ityfuzz evm -t 'tests/evm/multi-contract/*' --constructor-args "ContractName:arg1,arg2,...;AnotherContract:arg1,arg2,..;"
```

For example, if you have two contracts, `main` and `main2`, both of which have a `bytes32` and a `uint256` as constructor parameters, you can pass them in like this:

```bash
ityfuzz evm -t 'tests/evm/multi-contract/*' --constructor-args "main:1,0x6100000000000000000000000000000000000000000000000000000000000000;main2:2,0x6200000000000000 000000000000000000000000000000000000000000000000;"
```

**Method 2: Server forwarding**

The second method is to use our server to forward the request to a user-specified RPC, and the cli will get the constructor parameters from the transaction sent to the RPC.

First, go to the `/server` directory and install the necessary packages:

```bash
cd /server
npm install
```

Then, start the server using the following command:

```bash
node app.js
```

By default, the server forwards requests to `http://localhost:8545`, which is the default address of [Ganache](https://github.com/trufflesuite/ganache). If you are not running a local blockchain, You can start one using Ganache.
If you wish to forward the request to another location, you can specify the address as a command line argument like this:

```bash
node app.js http://localhost:8546
```

Once the server is running, you can use the tool of your choice to deploy your contract to
`localhost:5001`。

For example, you can use Foundry to deploy your contract via a server:

```bash
forge create src/flashloan.sol:main2 --rpc-url http://127.0.0.1:5001 --private-key 0x0000000000000000000000000000000000000000000000000000000000000000 --constructor-args "1" "0x6100000000000000000000000000000000000000000000000000000000000000"
```

Finally, you can get the constructor arguments using the `--fetch-tx-data` flag:

```bash
ityfuzz evm -t 'tests/evm/multi-contract/*' --fetch-tx-data
```

ItyFuzz will get the constructor parameters from the transaction forwarded to RPC by the server.

### Z3 installation

**macOS**

```bash
git clone https://github.com/Z3Prover/z3 && cd z3
python scripts/mk_make.py --prefix=/usr/local
cd build && make -j64 && sudo make install
```

If the build command still fails because `z3.h` is not found, execute `export Z3_SYS_Z3_HEADER=/usr/local/include/z3.h`

Or you can use

```bash
brew install z3
```

**Ubuntu**

```bash
apt install libz3-dev
```

### Citation

```
@misc{ityfuzz,
       title={ItyFuzz: Snapshot-Based Fuzzer for Smart Contract},
       author={Chaofan Shou and Shangyin Tan and Koushik Sen},
       year={2023},
       eprint={2306.17135},
       archivePrefix={arXiv},
       primaryClass={cs.CR}
}
```
