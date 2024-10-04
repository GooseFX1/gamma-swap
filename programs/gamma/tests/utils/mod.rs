#![allow(dead_code)]

use anchor_spl::associated_token::{get_associated_token_address, spl_associated_token_account};
use anchor_spl::token::TokenAccount;
use anchor_spl::token_2022::spl_token_2022;
use anchor_spl::token_interface::spl_token_metadata_interface::borsh::BorshDeserialize;
use anchor_spl::token_interface::spl_token_metadata_interface::state::TokenMetadata;
use gamma::curve::TradeDirection;
use gamma::states::{
    ObservationState, AMM_CONFIG_SEED, OBSERVATION_NUM, OBSERVATION_SEED,
    POOL_LP_MINT_SEED, POOL_SEED, POOL_VAULT_SEED, USER_POOL_LIQUIDITY_SEED,
};
use gamma::AUTH_SEED;
use solana_program_runtime::invoke_context::BuiltinFunctionWithContext;
use solana_sdk::instruction::Instruction;
use solana_sdk::program_option::COption;
use solana_sdk::program_pack::Pack;

use anchor_lang::prelude::{Clock, Pubkey, Rent};
use anchor_lang::{system_program, AccountDeserialize, InstructionData, ToAccountMetas};
use solana_program_test::{
    BanksClientError, BanksTransactionResultWithMetadata, ProgramTest, ProgramTestBanksClientExt,
    ProgramTestContext,
};
use solana_sdk::account::Account;
use solana_sdk::account::ReadableAccount;
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;
use solana_sdk::transaction::Transaction;
use solana_sdk::{system_transaction, sysvar};

pub type ProcessTransactionResult = Result<BanksTransactionResultWithMetadata, BanksClientError>;

pub trait ExpectTransaction {
    fn expect_transaction(&self, msg: &str);
    fn unwrap_transaction(&self);
}
impl ExpectTransaction for ProcessTransactionResult {
    #[inline]
    #[track_caller]
    fn expect_transaction(&self, msg: &str) {
        let transaction_result_data = self
            .as_ref()
            .expect("Failed to get transaction return data");
        transaction_result_data.result.as_ref().expect(msg);
    }

    #[inline]
    #[track_caller]
    fn unwrap_transaction(&self) {
        self.expect_transaction("Failed to perform transaction");
    }
}

pub struct TestEnv {
    pub token_0_mint: Pubkey,
    pub token_1_mint: Pubkey,
    pub mint_authority: Keypair,
    pub program_test_context: ProgramTestContext,
    pub treasury: Pubkey,
}

pub const TEST_ADMIN_KEYPAIR: [u8; 64] = [
    197, 168, 140, 152, 235, 128, 10, 88, 6, 137, 32, 129, 32, 24, 16, 12, 151, 42, 128, 206, 33,
    170, 155, 53, 149, 95, 159, 133, 74, 145, 201, 141, 174, 47, 245, 164, 204, 44, 214, 85, 145,
    45, 61, 4, 6, 167, 148, 235, 184, 142, 47, 7, 141, 43, 137, 163, 155, 196, 128, 175, 71, 162,
    129, 206,
];

mod wallet {
    use solana_sdk::signature::{read_keypair_file, Keypair};
    use static_init::dynamic;

    #[allow(non_camel_case_types)]
    #[dynamic]
    pub static WALLET: Keypair = {
        let home = std::env::var("HOME").unwrap();
        read_keypair_file(format!("{home}/.config/solana/id.json"))
            .expect("Please configure your system and add id.json")
    };
}

pub fn get_wallet() -> Keypair {
    // Insecure clone can be used in tests. It is not recommended to make copy of secret keys in memory.
    wallet::WALLET.insecure_clone()
}

pub fn get_admin() -> Keypair {
    Keypair::from_bytes(&TEST_ADMIN_KEYPAIR).unwrap()
}

pub fn get_instruction<AnchorInstruction, AnchorAccounts>(
    data: AnchorInstruction,
    accounts: AnchorAccounts,
) -> Instruction
where
    AnchorInstruction: InstructionData,
    AnchorAccounts: ToAccountMetas,
{
    get_instruction_with_program_id(data, accounts, gamma::ID)
}

pub fn get_instruction_with_program_id<AnchorInstruction, AnchorAccounts>(
    data: AnchorInstruction,
    accounts: AnchorAccounts,
    program_id: Pubkey,
) -> Instruction
where
    AnchorInstruction: InstructionData,
    AnchorAccounts: ToAccountMetas,
{
    Instruction {
        program_id,
        data: data.data(),
        accounts: accounts.to_account_metas(None),
    }
}

pub async fn get_signed_transaction(
    program_context: &mut ProgramTestContext,
    instructions: &[Instruction],
    signer: &Keypair,
) -> Transaction {
    let blockhash = program_context
        .banks_client
        .get_latest_blockhash()
        .await
        .unwrap();
    let recent_blockhash = program_context
        .banks_client
        .get_new_latest_blockhash(&blockhash)
        .await
        .unwrap();

    let mut tx = Transaction::new_with_payer(instructions, Some(&signer.pubkey()));
    tx.partial_sign(&[signer], recent_blockhash);
    tx
}

pub async fn get_signed_transaction_with_different_payer(
    program_context: &mut ProgramTestContext,
    instructions: &[Instruction],
    signer: &Keypair,
    payer: &Keypair,
) -> Transaction {
    let blockhash = program_context
        .banks_client
        .get_latest_blockhash()
        .await
        .unwrap();
    let recent_blockhash = program_context
        .banks_client
        .get_new_latest_blockhash(&blockhash)
        .await
        .unwrap();

    let mut tx = Transaction::new_with_payer(instructions, Some(&payer.pubkey()));
    tx.partial_sign(&[signer, payer], recent_blockhash);
    tx
}

#[macro_export]
macro_rules! assert_error {
    ($transaction_result: ident, $credix_error: expr) => {{
        use anchor_lang::error;
        use anchor_lang::prelude::ProgramError;
        use solana_sdk::transaction::TransactionError;
        use std::convert::TryFrom;

        let transaction_result = $transaction_result.unwrap().result;
        let error = {
            if transaction_result.is_ok() {
                panic!("Test case did not return error");
            }
            let transaction_error = transaction_result.err().unwrap();

            match transaction_error {
                TransactionError::InstructionError(_, err) => err,
                _ => panic!("Unexpected error"),
            }
        };
        let program_err = ProgramError::try_from(error).unwrap();
        let expected_error: ProgramError = error!($credix_error).into();

        assert_eq!(program_err, expected_error);
    }};
}

// We are using packed struct, See: https://github.com/rust-lang/rust/issues/82523
#[macro_export]
macro_rules! assert_eq_with_copy {
    ($left:expr, $right:expr $(,)?) => {
        assert_eq!({ $left }, { $right })
    };
}

pub const INITIAL_ACCOUNT_LAMPORTS: u64 = 10_000_000_000_000;

pub fn get_current_price_token_0_price(observation: ObservationState) -> u128 {
    let current_observation_index = observation.observation_index as usize;
    let last_observation_index = match current_observation_index {
        0 => OBSERVATION_NUM - 1,
        _ => current_observation_index - 1,
    };
    let delta_time = observation.observations[current_observation_index]
        .block_timestamp
        .saturating_sub(observation.observations[last_observation_index].block_timestamp);

    (observation.observations[current_observation_index].cumulative_token_0_price_x32
        - observation.observations[last_observation_index].cumulative_token_0_price_x32)
        .checked_div(delta_time.into())
        .unwrap()
}

pub struct ProgramInfo {
    pub program_name: String,
    pub program_id: Pubkey,
    pub process_instruction: Option<BuiltinFunctionWithContext>,
}

pub struct AccountConfigInfo {
    pub address: Pubkey,
    pub data: Vec<u8>,
    pub owner: Pubkey,
}

pub struct AccountConfigInfoBase64 {
    pub address: Pubkey,
    pub data: String,
    pub owner: Pubkey,
}

impl TestEnv {
    pub async fn new_with_config(mut accounts: Vec<Pubkey>, programs: Vec<ProgramInfo>) -> TestEnv {
        let mut program_test = ProgramTest::new("gamma", gamma::id(), None);

        for program in programs {
            program_test.add_program(
                &program.program_name,
                program.program_id,
                program.process_instruction,
            );
        }

        // TODO: Add metadata program
        // program_test.add_program(
        //     "../../artifacts/metadata_program",
        //     gamma::program_utils::mpl_token_metadata::ID,
        //     None,
        // );

        accounts.push(get_wallet().pubkey());
        let mint_authority = Keypair::new();
        accounts.push(mint_authority.pubkey());

        accounts.iter().for_each(|pubkey| {
            program_test.add_account(
                pubkey.to_owned(),
                Account {
                    lamports: INITIAL_ACCOUNT_LAMPORTS,
                    ..Default::default()
                },
            )
        });

        let mut token_account_data = vec![0; 165];

        let data = spl_token::state::Account {
            mint: Pubkey::new_unique(),
            owner: Pubkey::new_unique(),
            amount: 0,
            delegate: COption::None,
            state: spl_token::state::AccountState::Initialized,
            is_native: COption::None,
            delegated_amount: 0,
            close_authority: COption::None,
        };

        data.pack_into_slice(&mut token_account_data);

        program_test.add_account(
            gamma::create_pool_fee_reveiver::ID,
            Account {
                lamports: INITIAL_ACCOUNT_LAMPORTS,
                owner: spl_token::id(),
                data: token_account_data,
                ..Default::default()
            },
        );

        let context = program_test.start_with_context().await;

        let mints = [Keypair::new(), Keypair::new()];
        let (token0, token1) = if mints[0].pubkey() < mints[1].pubkey() {
            (mints[0].insecure_clone(), mints[1].insecure_clone())
        } else {
            (mints[1].insecure_clone(), mints[0].insecure_clone())
        };

        let mut testenv = TestEnv {
            program_test_context: context,
            mint_authority,
            token_0_mint: token0.pubkey(),
            token_1_mint: token1.pubkey(),
            treasury: Pubkey::new_unique(),
        };

        testenv
            .create_token_mint(&token0, &testenv.mint_authority.pubkey(), 6)
            .await;
        testenv
            .create_token_mint(&token1, &testenv.mint_authority.pubkey(), 6)
            .await;

        testenv
    }

    pub async fn new(accounts: Vec<Pubkey>) -> TestEnv {
        TestEnv::new_with_config(accounts, vec![]).await
    }

    pub async fn create_token_mint(
        &mut self,
        token_mint: &Keypair,
        authority: &Pubkey,
        decimals: u8,
    ) {
        let latest_blockhash = self
            .program_test_context
            .banks_client
            .get_latest_blockhash()
            .await
            .unwrap();
        self.program_test_context
            .banks_client
            .process_transaction(system_transaction::create_account(
                &get_wallet(),
                token_mint,
                latest_blockhash,
                Rent::default().minimum_balance(spl_token::state::Mint::get_packed_len()),
                spl_token::state::Mint::get_packed_len() as u64,
                &spl_token::id(),
            ))
            .await
            .unwrap();

        let ix = spl_token::instruction::initialize_mint(
            &spl_token::id(),
            &token_mint.pubkey(),
            authority,
            None,
            decimals,
        )
        .unwrap();

        self.program_test_context
            .banks_client
            .process_transaction(Transaction::new_signed_with_payer(
                &[ix],
                Some(&get_wallet().pubkey()),
                &[&get_wallet()],
                latest_blockhash,
            ))
            .await
            .expect("Failed to create token mint");
    }

    pub async fn create_token_account(
        &mut self,
        account: &Keypair,
        authority: &Pubkey,
        mint: &Pubkey,
        payer: &Keypair,
    ) {
        let latest_blockhash = self
            .program_test_context
            .banks_client
            .get_latest_blockhash()
            .await
            .unwrap();
        self.program_test_context
            .banks_client
            .process_transaction(system_transaction::create_account(
                payer,
                account,
                latest_blockhash,
                Rent::default().minimum_balance(spl_token::state::Account::get_packed_len()),
                spl_token::state::Account::get_packed_len() as u64,
                &spl_token::id(),
            ))
            .await
            .unwrap();

        let ix = spl_token::instruction::initialize_account(
            &spl_token::id(),
            &account.pubkey(),
            mint,
            authority,
        )
        .unwrap();

        self.program_test_context
            .banks_client
            .process_transaction(Transaction::new_signed_with_payer(
                &[ix],
                Some(&get_wallet().pubkey()),
                &[&get_wallet()],
                latest_blockhash,
            ))
            .await
            .unwrap();
    }

    pub async fn create_associated_token_account(
        &mut self,
        account: &Pubkey,
        mint: &Pubkey,
        payer: &Keypair,
    ) -> Pubkey {
        let latest_blockhash = self
            .program_test_context
            .banks_client
            .get_latest_blockhash()
            .await
            .unwrap();
        let associated_token_account = get_associated_token_address(account, mint);
        let ix = spl_associated_token_account::instruction::create_associated_token_account(
            &payer.pubkey(),
            account,
            mint,
            &anchor_spl::token::ID,
        );

        self.program_test_context
            .banks_client
            .process_transaction(Transaction::new_signed_with_payer(
                &[ix],
                Some(&payer.pubkey()),
                &[payer],
                latest_blockhash,
            ))
            .await
            .unwrap();

        associated_token_account
    }

    pub async fn get_or_create_associated_token_account(
        &mut self,
        account: Pubkey,
        mint: Pubkey,
        payer: &Keypair,
    ) -> Pubkey {
        let associated_token_account = get_associated_token_address(&account, &mint);

        let existing_account: Result<TokenAccount, BanksClientError> =
            self.try_fetch_account(associated_token_account).await;
        if existing_account.is_ok() {
            return associated_token_account;
        }

        self.create_associated_token_account(&account, &mint, payer)
            .await
    }

    pub async fn timestamp_now(&mut self) -> i64 {
        let clock: Clock = self
            .program_test_context
            .banks_client
            .get_sysvar()
            .await
            .unwrap();
        clock.unix_timestamp
    }

    pub async fn get_account_info(
        &mut self,
        address: Pubkey,
    ) -> Result<Option<Account>, BanksClientError> {
        self.program_test_context
            .banks_client
            .get_account(address)
            .await
    }

    pub async fn fetch_account<T: AccountDeserialize>(&mut self, address: Pubkey) -> T {
        self.try_fetch_account(address).await.unwrap()
    }

    pub async fn try_fetch_account<T: AccountDeserialize>(
        &mut self,
        address: Pubkey,
    ) -> Result<T, BanksClientError> {
        let result = self.get_account_info(address).await;
        let account = result?.ok_or(BanksClientError::ClientError("Account not found"))?;
        T::try_deserialize(&mut account.data.as_ref())
            .map_err(|_| BanksClientError::ClientError("Failed to deserialize account"))
    }

    pub async fn fetch_meta_token(&mut self, address: Pubkey) -> TokenMetadata {
        let account = self
            .get_account_info(address)
            .await
            .expect("unable to fetch account")
            .expect("unable to unwrap the account");

        let m = TokenMetadata::deserialize(&mut &account.data()[(32 + 32 + 1)..])
            .expect("unable to deserialize into Metadata");
        m
    }

    pub async fn mint_base_tokens(
        &mut self,
        token_account: Pubkey,
        amount: u64,
        token_mint: Pubkey,
    ) {
        let mint_ix = anchor_spl::token::spl_token::instruction::mint_to(
            &anchor_spl::token::spl_token::id(),
            &token_mint,
            &token_account,
            &self.mint_authority.pubkey(),
            &[&self.mint_authority.pubkey()],
            amount,
        )
        .expect("Failed to create mint instruction");

        let mint_tx = get_signed_transaction(
            &mut self.program_test_context,
            &[mint_ix],
            &self.mint_authority,
        )
        .await;

        self.program_test_context
            .banks_client
            .process_transaction(mint_tx)
            .await
            .expect("Failed to mint base tokens");
    }

    pub async fn jump_seconds<T: Into<i64>>(&mut self, jump_seconds: T) {
        // we add 1 seconds so we are in the new time(computers are very fast).
        let timestamp = self.timestamp_now().await + jump_seconds.into() + 1;
        self.warp_to_timestamp(timestamp).await;
    }

    pub async fn jump_days<T: Into<i64>>(&mut self, jump_days: T) {
        // we do -1 here because we want to stay in the same day.
        let seconds = 60 * 60 * 24 * jump_days.into() - 1;
        self.jump_seconds(seconds).await;
    }

    pub async fn warp_to_timestamp(&mut self, timestamp: i64) {
        const NANOSECONDS_IN_SECOND: i64 = 1_000_000_000;

        let mut clock: Clock = self
            .program_test_context
            .banks_client
            .get_sysvar()
            .await
            .unwrap();
        let now = clock.unix_timestamp;
        let current_slot = clock.slot;
        clock.unix_timestamp = timestamp;

        if now >= timestamp {
            panic!("Timestamp incorrect. Cannot set time backwards.")
        }

        let ns_per_slot = self.program_test_context.genesis_config().ns_per_slot();
        let timestamp_diff_ns = timestamp
            .checked_sub(now) //calculate time diff
            .expect("Problem with timestamp diff calculation.")
            .checked_mul(NANOSECONDS_IN_SECOND) //convert from s to ns
            .expect("Problem with timestamp diff calculation.")
            as u128;

        let slots = timestamp_diff_ns
            .checked_div(ns_per_slot)
            .expect("Problem with slots from timestamp calculation.") as u64;

        self.program_test_context.set_sysvar(&clock);
        self.program_test_context
            .warp_to_slot(current_slot + slots)
            .unwrap();
    }

    pub async fn encode_instruction_and_sign_transaction<AnchorInstruction, AnchorAccounts>(
        &mut self,
        data: AnchorInstruction,
        accounts: AnchorAccounts,
        signer: &Keypair,
    ) -> Transaction
    where
        AnchorInstruction: InstructionData,
        AnchorAccounts: ToAccountMetas,
    {
        let instruction = get_instruction(data, accounts);
        get_signed_transaction(&mut self.program_test_context, &[instruction], &signer).await
    }

    pub async fn create_config(
        &mut self,
        user: &Keypair,
        amm_index: u16,
        trade_fee_rate: u64,
        protocol_fee_rate: u64,
        fund_fee_rate: u64,
        create_pool_fee: u64,
    ) {
        let (amm_config_key, __bump) = Pubkey::find_program_address(
            &[AMM_CONFIG_SEED.as_bytes(), &amm_index.to_be_bytes()],
            &gamma::ID,
        );

        let accounts = gamma::accounts::CreateAmmConfig {
            owner: user.pubkey(),
            amm_config: amm_config_key,
            system_program: system_program::ID,
        };

        let data = gamma::instruction::CreateAmmConfig {
            index: amm_index,
            trade_fee_rate,
            protocol_fee_rate,
            fund_fee_rate,
            create_pool_fee,
        };

        let transaction = self
            .encode_instruction_and_sign_transaction(data, accounts, user)
            .await;

        self.program_test_context
            .banks_client
            .process_transaction(transaction)
            .await
            .unwrap();
    }

    pub async fn initialize_pool(
        &mut self,
        user: &Keypair,
        amm_config_index: u16,
        init_amount_0: u64,
        init_amount_1: u64,
        open_time: u64,
        create_pool_fee: Pubkey,
    ) -> Pubkey {
        let (amm_config_key, __bump) = Pubkey::find_program_address(
            &[AMM_CONFIG_SEED.as_bytes(), &amm_config_index.to_be_bytes()],
            &gamma::ID,
        );

        let pool_account_key = Pubkey::find_program_address(
            &[
                POOL_SEED.as_bytes(),
                amm_config_key.to_bytes().as_ref(),
                self.token_0_mint.to_bytes().as_ref(),
                self.token_1_mint.to_bytes().as_ref(),
            ],
            &gamma::ID,
        ).0;
        let (authority, __bump) = Pubkey::find_program_address(&[AUTH_SEED.as_bytes()], &gamma::ID);
        let (token_0_vault, __bump) = Pubkey::find_program_address(
            &[
                POOL_VAULT_SEED.as_bytes(),
                pool_account_key.to_bytes().as_ref(),
                self.token_0_mint.to_bytes().as_ref(),
            ],
            &gamma::ID,
        );
        let (token_1_vault, __bump) = Pubkey::find_program_address(
            &[
                POOL_VAULT_SEED.as_bytes(),
                pool_account_key.to_bytes().as_ref(),
                self.token_1_mint.to_bytes().as_ref(),
            ],
            &gamma::ID,
        );
        let (_lp_mint_key, __bump) = Pubkey::find_program_address(
            &[
                POOL_LP_MINT_SEED.as_bytes(),
                pool_account_key.to_bytes().as_ref(),
            ],
            &gamma::ID,
        );
        let (observation_key, __bump) = Pubkey::find_program_address(
            &[
                OBSERVATION_SEED.as_bytes(),
                pool_account_key.to_bytes().as_ref(),
            ],
            &gamma::ID,
        );
        let user_token_0_account = self
            .get_or_create_associated_token_account(user.pubkey(), self.token_0_mint.clone(), &user)
            .await;

        let user_token_1_account = self
            .get_or_create_associated_token_account(user.pubkey(), self.token_1_mint.clone(), &user)
            .await;
        let user_pool_liquidity = Pubkey::find_program_address(
            &[
                USER_POOL_LIQUIDITY_SEED.as_bytes(),
                pool_account_key.to_bytes().as_ref(),
                user.pubkey().to_bytes().as_ref(),
            ],
            &gamma::ID,
        )
        .0;

        let accounts = gamma::accounts::Initialize {
            creator: user.pubkey(),
            amm_config: amm_config_key,
            authority,
            pool_state: pool_account_key,
            user_pool_liquidity,
            token_0_mint: self.token_0_mint,
            token_1_mint: self.token_1_mint,
            creator_token_0: user_token_0_account,
            creator_token_1: user_token_1_account,
            token_0_vault,
            token_1_vault,
            create_pool_fee: create_pool_fee,
            observation_state: observation_key,
            token_program: spl_token::id(),
            token_0_program: spl_token::id(),
            token_1_program: spl_token::id(),
            associated_token_program: spl_associated_token_account::id(),
            system_program: system_program::ID,
            rent: sysvar::rent::id(),
        };

        let data = gamma::instruction::Initialize {
            init_amount_0,
            init_amount_1,
            open_time,
        };

        let transaction = self
            .encode_instruction_and_sign_transaction(data, accounts, user)
            .await;

        self.program_test_context
            .banks_client
            .process_transaction(transaction)
            .await
            .unwrap();

        pool_account_key
    }

    pub async fn deposit(
        &mut self,
        user: &Keypair,
        pool_id: Pubkey,
        amm_config_index: u16,
        lp_token_amount: u64,
        maximum_token_0_amount: u64,
        maximum_token_1_amount: u64,
    ) {
        let (amm_config_key, __bump) = Pubkey::find_program_address(
            &[AMM_CONFIG_SEED.as_bytes(), &amm_config_index.to_be_bytes()],
            &gamma::ID,
        );

        let (pool_account_key, __bump) = Pubkey::find_program_address(
            &[
                POOL_SEED.as_bytes(),
                amm_config_key.to_bytes().as_ref(),
                self.token_0_mint.to_bytes().as_ref(),
                self.token_1_mint.to_bytes().as_ref(),
            ],
            &gamma::ID,
        );

        let (token_0_vault, __bump) = Pubkey::find_program_address(
            &[
                POOL_VAULT_SEED.as_bytes(),
                pool_account_key.to_bytes().as_ref(),
                self.token_0_mint.to_bytes().as_ref(),
            ],
            &gamma::ID,
        );
        let (token_1_vault, __bump) = Pubkey::find_program_address(
            &[
                POOL_VAULT_SEED.as_bytes(),
                pool_account_key.to_bytes().as_ref(),
                self.token_1_mint.to_bytes().as_ref(),
            ],
            &gamma::ID,
        );

        let user_token_0_account = self
            .get_or_create_associated_token_account(user.pubkey(), self.token_0_mint.clone(), &user)
            .await;

        let user_token_1_account = self
            .get_or_create_associated_token_account(user.pubkey(), self.token_1_mint.clone(), &user)
            .await;

        let (authority, __bump) =
            Pubkey::find_program_address(&[AUTH_SEED.as_bytes()], &gamma::id());

        let user_pool_liquidity = Pubkey::find_program_address(
            &[
                USER_POOL_LIQUIDITY_SEED.as_bytes(),
                pool_id.to_bytes().as_ref(),
                user.pubkey().to_bytes().as_ref(),
            ],
            &gamma::id(),
        )
        .0;

        let accounts = gamma::accounts::Deposit {
            owner: user.pubkey(),
            authority,
            pool_state: pool_id,
            user_pool_liquidity,
            // owner_lp_token: user_token_lp_account,
            token_0_account: user_token_0_account,
            token_1_account: user_token_1_account,
            token_0_vault,
            token_1_vault,
            token_program: spl_token::id(),
            token_program_2022: spl_token_2022::id(),
            vault_0_mint: self.token_0_mint,
            vault_1_mint: self.token_1_mint,
        };

        let data = gamma::instruction::Deposit {
            lp_token_amount,
            maximum_token_0_amount,
            maximum_token_1_amount,
        };

        let transaction = self
            .encode_instruction_and_sign_transaction(data, accounts, user)
            .await;

        self.program_test_context
            .banks_client
            .process_transaction(transaction)
            .await
            .unwrap();
    }

    pub async fn withdraw(
        &mut self,
        user: &Keypair,
        pool_id: Pubkey,
        amm_config_index: u16,
        lp_token_amount: u64,
        minimum_token_0_amount: u64,
        minimum_token_1_amount: u64,
    ) {
        let (amm_config_key, __bump) = Pubkey::find_program_address(
            &[AMM_CONFIG_SEED.as_bytes(), &amm_config_index.to_be_bytes()],
            &gamma::ID,
        );

        let (pool_account_key, __bump) = Pubkey::find_program_address(
            &[
                POOL_SEED.as_bytes(),
                amm_config_key.to_bytes().as_ref(),
                self.token_0_mint.to_bytes().as_ref(),
                self.token_1_mint.to_bytes().as_ref(),
            ],
            &gamma::ID,
        );

        let (token_0_vault, __bump) = Pubkey::find_program_address(
            &[
                POOL_VAULT_SEED.as_bytes(),
                pool_account_key.to_bytes().as_ref(),
                self.token_0_mint.to_bytes().as_ref(),
            ],
            &gamma::ID,
        );
        let (token_1_vault, __bump) = Pubkey::find_program_address(
            &[
                POOL_VAULT_SEED.as_bytes(),
                pool_account_key.to_bytes().as_ref(),
                self.token_1_mint.to_bytes().as_ref(),
            ],
            &gamma::ID,
        );
        let (authority, __bump) =
            Pubkey::find_program_address(&[AUTH_SEED.as_bytes()], &gamma::id());
        let user_pool_liquidity = Pubkey::find_program_address(
            &[
                USER_POOL_LIQUIDITY_SEED.as_bytes(),
                pool_id.to_bytes().as_ref(),
                user.pubkey().to_bytes().as_ref(),
            ],
            &gamma::id(),
        )
        .0;

        let user_token_0_account: Pubkey = self
            .get_or_create_associated_token_account(user.pubkey(), self.token_0_mint.clone(), &user)
            .await;

        let user_token_1_account = self
            .get_or_create_associated_token_account(user.pubkey(), self.token_1_mint.clone(), &user)
            .await;

        let accounts = gamma::accounts::Withdraw {
            owner: user.pubkey(),
            authority,
            pool_state: pool_id,
            user_pool_liquidity,
            token_0_account: user_token_0_account,
            token_1_account: user_token_1_account,
            token_0_vault,
            token_1_vault,
            token_program: spl_token::id(),
            token_program_2022: spl_token_2022::id(),
            vault_0_mint: self.token_0_mint,
            vault_1_mint: self.token_1_mint,
            memo_program: spl_memo::id(),
        };

        let data = gamma::instruction::Withdraw {
            lp_token_amount,
            minimum_token_0_amount,
            minimum_token_1_amount,
        };

        let transaction = self
            .encode_instruction_and_sign_transaction(data, accounts, user)
            .await;

        self.program_test_context
            .banks_client
            .process_transaction(transaction)
            .await
            .unwrap();
    }

    pub async fn init_user_pool_liquidity(&mut self, user: &Keypair, pool_id: Pubkey) {
        let user_pool_liquidity = Pubkey::find_program_address(
            &[
                USER_POOL_LIQUIDITY_SEED.as_bytes(),
                pool_id.to_bytes().as_ref(),
                user.pubkey().to_bytes().as_ref(),
            ],
            &gamma::id(),
        )
        .0;

        let accounts = gamma::accounts::InitUserPoolLiquidity {
            user: user.pubkey(),
            pool_state: pool_id,
            user_pool_liquidity,
            system_program: system_program::ID,
        };

        let data = gamma::instruction::InitUserPoolLiquidity {};

        let transaction = self
            .encode_instruction_and_sign_transaction(data, accounts, user)
            .await;

        self.program_test_context
            .banks_client
            .process_transaction(transaction)
            .await
            .unwrap();
    }

    pub async fn swap_base_input(
        &mut self,
        user: &Keypair,
        pool_id: Pubkey,
        amm_config_index: u16,
        amount_in: u64,
        minimum_amount_out: u64,
        trade_direction: TradeDirection,
    ) {
        let (authority, __bump) =
            Pubkey::find_program_address(&[AUTH_SEED.as_bytes()], &gamma::id());
        let (amm_config_key, __bump) = Pubkey::find_program_address(
            &[AMM_CONFIG_SEED.as_bytes(), &amm_config_index.to_be_bytes()],
            &gamma::ID,
        );

        let user_token_0_account: Pubkey = self
            .get_or_create_associated_token_account(user.pubkey(), self.token_0_mint.clone(), &user)
            .await;

        let user_token_1_account = self
            .get_or_create_associated_token_account(user.pubkey(), self.token_1_mint.clone(), &user)
            .await;

        let (token_0_vault, __bump) = Pubkey::find_program_address(
            &[
                POOL_VAULT_SEED.as_bytes(),
                pool_id.to_bytes().as_ref(),
                self.token_0_mint.to_bytes().as_ref(),
            ],
            &gamma::ID,
        );
        let (token_1_vault, __bump) = Pubkey::find_program_address(
            &[
                POOL_VAULT_SEED.as_bytes(),
                pool_id.to_bytes().as_ref(),
                self.token_1_mint.to_bytes().as_ref(),
            ],
            &gamma::ID,
        );
        let (observation_key, __bump) = Pubkey::find_program_address(
            &[OBSERVATION_SEED.as_bytes(), pool_id.to_bytes().as_ref()],
            &gamma::ID,
        );

        let (
            input_token_account,
            output_token_account,
            input_token_mint,
            output_token_mint,
            input_vault,
            output_vault,
            input_token_program,
            output_token_program,
        ) = match trade_direction {
            TradeDirection::ZeroForOne => (
                user_token_0_account,
                user_token_1_account,
                self.token_0_mint,
                self.token_1_mint,
                token_0_vault,
                token_1_vault,
                spl_token::id(),
                spl_token::id(),
            ),
            TradeDirection::OneForZero => (
                user_token_1_account,
                user_token_0_account,
                self.token_1_mint,
                self.token_0_mint,
                token_1_vault,
                token_0_vault,
                spl_token::id(),
                spl_token::id(),
            ),
        };

        let accounts = gamma::accounts::Swap {
            payer: user.pubkey(),
            authority,
            amm_config: amm_config_key,
            pool_state: pool_id,
            observation_state: observation_key,
            input_token_account,
            output_token_account,
            input_vault,
            output_vault,
            input_token_program,
            output_token_program,
            input_token_mint,
            output_token_mint,
        };

        let data = gamma::instruction::SwapBaseInput {
            amount_in,
            minimum_amount_out,
        };

        let transaction = self
            .encode_instruction_and_sign_transaction(data, accounts, user)
            .await;

        self.program_test_context
            .banks_client
            .process_transaction(transaction)
            .await
            .unwrap();
    }

    pub async fn swap_base_output(
        &mut self,
        user: &Keypair,
        pool_id: Pubkey,
        amm_config_index: u16,
        amount_out: u64,
        max_amount_in: u64,
    ) {
        let (authority, __bump) =
            Pubkey::find_program_address(&[AUTH_SEED.as_bytes()], &gamma::id());
        let (amm_config_key, __bump) = Pubkey::find_program_address(
            &[AMM_CONFIG_SEED.as_bytes(), &amm_config_index.to_be_bytes()],
            &gamma::ID,
        );

        let user_token_0_account: Pubkey = self
            .get_or_create_associated_token_account(user.pubkey(), self.token_0_mint.clone(), &user)
            .await;

        let user_token_1_account = self
            .get_or_create_associated_token_account(user.pubkey(), self.token_1_mint.clone(), &user)
            .await;

        let (token_0_vault, __bump) = Pubkey::find_program_address(
            &[
                POOL_VAULT_SEED.as_bytes(),
                pool_id.to_bytes().as_ref(),
                self.token_0_mint.to_bytes().as_ref(),
            ],
            &gamma::ID,
        );
        let (token_1_vault, __bump) = Pubkey::find_program_address(
            &[
                POOL_VAULT_SEED.as_bytes(),
                pool_id.to_bytes().as_ref(),
                self.token_1_mint.to_bytes().as_ref(),
            ],
            &gamma::ID,
        );
        let (observation_key, __bump) = Pubkey::find_program_address(
            &[OBSERVATION_SEED.as_bytes(), pool_id.to_bytes().as_ref()],
            &gamma::ID,
        );

        let accounts = gamma::accounts::Swap {
            payer: user.pubkey(),
            authority,
            amm_config: amm_config_key,
            pool_state: pool_id,
            input_token_account: user_token_0_account,
            output_token_account: user_token_1_account,
            input_vault: token_0_vault,
            output_vault: token_1_vault,
            input_token_program: spl_token::id(),
            output_token_program: spl_token::id(),
            input_token_mint: self.token_0_mint,
            output_token_mint: self.token_1_mint,
            observation_state: observation_key,
        };

        let data = gamma::instruction::SwapBaseOutput {
            amount_out,
            max_amount_in,
        };

        let transaction = self
            .encode_instruction_and_sign_transaction(data, accounts, user)
            .await;

        self.program_test_context
            .banks_client
            .process_transaction(transaction)
            .await
            .unwrap();
    }
}
