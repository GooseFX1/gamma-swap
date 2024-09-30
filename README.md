# Gamma

Gamma is a decentralized exchange (DEX) protocol built on Solana. It provides automated market making (AMM) functionality with customizable fee structures and liquidity pool management.

## Features

- Create and manage AMM configurations
- Initialize liquidity pools
- Deposit and withdraw liquidity
- Swap tokens with base input or base output
- Oracle price feed integration
- Transfer fee handling for SPL tokens(Token22 support)

## Project Structure

- `programs/gamma`: Solana program (smart contract) code
- `client`: Rust client for interacting with the Gamma program

## Getting Started

### Prerequisites

- Rust and Cargo
- Solana CLI tools
- Anchor framework

### Building

To build the project:
```bash
cargo make build_all
```

### Deploying

To deploy the program:
```bash
cargo make deploy_program
```

### Running the Client

The client provides a command-line interface for interacting with the Gamma program. Use the following command to see available options:
```bash
cargo install --path client
```

## Commands
```bash
gamma-cli --help
```

- `create-config`: Create a new AMM configuration
- `initialize-pool`: Initialize a new liquidity pool
- `init-user-pool-liquidity`: Initialize user pool liquidity account
- `deposit`: Deposit liquidity into a pool
- `withdraw`: Withdraw liquidity from a pool
- `swap-base-in`: Perform a token swap with a specified input amount
- `swap-base-out`: Perform a token swap with a specified output amount


### Testing

To run the test suite:
```bash
cargo test-sbf
```