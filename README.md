# Money Transfer Project Template (Rust)

This is a project for demonstrating Temporal with the [Rust SDK](https://github.com/temporalio/sdk-rust).

It implements a money transfer workflow that orchestrates withdrawing from a source account and depositing into a target account, with configurable retry policies and non-retryable error handling.

Based on the [Go money transfer template](https://github.com/temporalio/money-transfer-project-template-go).

## Project Structure

```
src/
├── shared.rs           # PaymentDetails struct and task queue constant
├── workflow.rs         # MoneyTransferWorkflow definition with retry policy
├── activity.rs         # Withdraw, deposit, and refund activities
├── banking_client.rs   # Mock banking service with in-memory accounts
├── worker/main.rs      # Worker binary — registers workflows and activities
└── start/main.rs       # Starter binary — kicks off a workflow execution

temporal-test-harness/  # Testing utility for running workflows without a server
tests/
├── activity_tests.rs           # Unit tests for banking activities
└── workflow_replay_tests.rs    # Workflow integration tests using mock history
```

## Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (edition 2024)
- A running [Temporal Server](https://docs.temporal.io/docs/server/quick-install/)

## Basic Instructions

### Step 0: Start Temporal Server

Make sure Temporal Server is running first. The quickest way is with the Temporal CLI:

```bash
temporal server start-dev
```

Or with Docker Compose:

```bash
git clone https://github.com/temporalio/docker-compose.git
cd docker-compose
docker-compose up
```

### Step 1: Clone this Repository

In another terminal instance, clone this repo:

```bash
git clone <repo-url>
cd temporal-rs-tutorial
```

### Step 2: Run the Workflow

```bash
cargo run --bin start
```

Observe that Temporal Web (http://localhost:8233) reflects the workflow, but it is still in "Running" status. This is because there is no Worker yet listening to the `TRANSFER_MONEY_TASK_QUEUE` task queue to process this work.

### Step 3: Run the Worker

In another terminal instance, run the worker. This worker hosts both the Workflow and Activity implementations.

```bash
cargo run --bin worker
```

Now you can see the workflow run to completion.

## Running Tests

The project includes unit tests for activities and workflow replay tests that run without a Temporal Server:

```bash
cargo test
```

Run specific test suites:

```bash
cargo test --test activity_tests
cargo test --test workflow_replay_tests
```

## How It Works

The `MoneyTransferWorkflow` executes two activities in sequence:

1. **Withdraw** — pulls funds from the source account
2. **Deposit** — deposits funds into the target account

Each activity is configured with a retry policy:

- Initial interval: 1 second
- Backoff coefficient: 2x
- Maximum interval: 5 seconds
- Maximum attempts: 5
- Non-retryable errors: `InvalidAccountError`, `InsufficientFundsError`

The workflow uses `rust_decimal::Decimal` for precise monetary calculations.
