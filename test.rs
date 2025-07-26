use candid::{decode_one, encode_args, Nat, Principal};
use icp_canister_backend::{Account, DepositError, TransferArg};
use pocket_ic::PocketIc;
use sha2::{Digest, Sha256};
mod types;
use types::*;

lazy_static::lazy_static! {
    static ref TRANSFER_FEE: Nat = Nat::from(10_000u64);
}

const ICRC1_LEDGER_WASM_PATH: &str = "../../src/icp_canister_backend/ic-icrc1-ledger.wasm";
const WASM_PATH: &str = "../../target/wasm32-unknown-unknown/release/icp_canister_backend.wasm";

fn setup() -> (PocketIc, Principal, Principal) {
    let pic = PocketIc::new();

    // Create and setup ICRC-1 ledger first
    let ledger_id = pic.create_canister();
    pic.add_cycles(ledger_id, 2_000_000_000_000);
    let ledger_wasm = std::fs::read(ICRC1_LEDGER_WASM_PATH).expect("ICRC-1 ledger WASM not found");

    // Setup ledger initialization
    let minting_account = Account {
        owner: ledger_id,
        subaccount: None,
    };

    let user = Principal::from_text("xkbqi-2qaaa-aaaah-qbpqq-cai").unwrap();
    let initial_balances = vec![(
        Account {
            owner: user,
            subaccount: None,
        },
        Nat::from(1_000_000_000u64), // 1000 tokens (assuming 6 decimals)
    )];

    let init_args = InitArgs {
        minting_account,
        fee_collector_account: None,
        transfer_fee: Nat::from(10_000u64),
        decimals: Some(6),
        max_memo_length: Some(64),
        token_symbol: "TEST".to_string(),
        token_name: "Test Token".to_string(),
        metadata: vec![],
        initial_balances,
        feature_flags: Some(FeatureFlags { icrc2: true }),
        maximum_number_of_accounts: None,
        accounts_overflow_trim_quantity: None,
        archive_options: ArchiveOptions {
            num_blocks_to_archive: 1000,
            max_transactions_per_response: Some(100),
            trigger_threshold: 2000,
            max_message_size_bytes: Some(1024 * 1024),
            cycles_for_archive_creation: Some(1_000_000_000_000),
            node_max_memory_size_bytes: Some(32 * 1024 * 1024),
            controller_id: ledger_id,
            more_controller_ids: None,
        },
    };

    // Install ledger with InitArgs wrapped in LedgerArg
    let ledger_arg = LedgerArg::Init(init_args);
    pic.install_canister(
        ledger_id,
        ledger_wasm,
        encode_args((ledger_arg,)).unwrap(),
        None,
    );

    // Create and setup main canister with ledger_id
    let canister_id = pic.create_canister();
    pic.add_cycles(canister_id, 2_000_000_000_000);
    let wasm = std::fs::read(WASM_PATH)
        .expect("Build first: cargo build --target wasm32-unknown-unknown --release");

    // Install canister with ledger_id as initial token_id
    let init_args = encode_args((ledger_id,)).unwrap();
    pic.install_canister(canister_id, wasm, init_args, None);

    (pic, canister_id, ledger_id)
}

#[test]
fn test_get_deposit_subaccount() {
    let (pic, canister_id, _) = setup();
    let user = Principal::from_text("xkbqi-2qaaa-aaaah-qbpqq-cai").unwrap();
    let timelock: u64 = 123456789;

    let result = pic
        .query_call(
            canister_id,
            user,
            "get_deposit_subaccount",
            encode_args((user.clone(), timelock)).unwrap(),
        )
        .expect("Failed to call get_deposit_subaccount");

    let returned_subaccount: [u8; 32] = decode_one(&result).unwrap();

    // Expected subaccount calculation
    let mut hasher = Sha256::new();
    hasher.update(user.as_slice());
    hasher.update(timelock.to_be_bytes());
    let expected_subaccount: [u8; 32] = hasher.finalize().into();

    assert_eq!(returned_subaccount, expected_subaccount);
}

#[test]
fn test_deposit_flow() {
    let (pic, canister_id, ledger_id) = setup();
    let user = Principal::from_text("xkbqi-2qaaa-aaaah-qbpqq-cai").unwrap();
    let timelock: u64 = 86400;
    let deposit_amount = Nat::from(100_000_000u64);

    //User calls get_deposit_subaccount
    let subaccount_result = pic
        .query_call(
            canister_id,
            user,
            "get_deposit_subaccount",
            encode_args((user, timelock)).unwrap(),
        )
        .expect("Failed to get deposit subaccount");

    let subaccount: [u8; 32] = decode_one(&subaccount_result).unwrap();

    //User transfers tokens to the subaccount
    let transfer_args = TransferArg {
        from_subaccount: None,
        to: Account {
            owner: canister_id,
            subaccount: Some(subaccount.to_vec()),
        },
        amount: deposit_amount.clone(),
        fee: Some(TRANSFER_FEE.clone()),
        memo: None,
        created_at_time: None,
    };

    let transfer_result = pic
        .update_call(
            ledger_id,
            user,
            "icrc1_transfer",
            encode_args((transfer_args,)).unwrap(),
        )
        .expect("Failed to transfer tokens");

    let transfer_result: TransferResult = decode_one(&transfer_result).unwrap();
    assert!(
        matches!(transfer_result, TransferResult::Ok(_)),
        "Transfer failed: {:?}",
        transfer_result
    );

    //User calls deposit function
    let deposit_result = pic
        .update_call(
            canister_id,
            user,
            "deposit",
            encode_args((user, timelock)).unwrap(),
        )
        .expect("Failed to call deposit");

    let result: Result<(), DepositError> = decode_one(&deposit_result).unwrap();
    assert!(result.is_ok(), "Deposit failed: {:?}", result);

    // Verify the canister main account has the tokens
    let main_account = Account {
        owner: canister_id,
        subaccount: None,
    };

    let balance_check = pic
        .query_call(
            ledger_id,
            user,
            "icrc1_balance_of",
            encode_args((main_account,)).unwrap(),
        )
        .expect("Failed to check canister balance");

    let canister_balance: Nat = decode_one(&balance_check).unwrap();
    let expected_balance = deposit_amount.clone() - TRANSFER_FEE.clone();
    assert_eq!(
        canister_balance, expected_balance,
        "Canister should have received the exact expected tokens"
    );
}

#[test]
fn test_deposit_fails_without_transfer() {
    let (pic, canister_id, _ledger_id) = setup();
    let user = Principal::from_text("xkbqi-2qaaa-aaaah-qbpqq-cai").unwrap();
    let timelock: u64 = 86400;
    // Directly call deposit
    let deposit_result = pic
        .update_call(
            canister_id,
            user,
            "deposit",
            encode_args((user, timelock)).unwrap(),
        )
        .expect("Failed to call deposit");

    let result: Result<(), DepositError> = decode_one(&deposit_result).unwrap();
    assert!(
        matches!(result, Err(DepositError::InsufficientBalance)),
        "Expected InsufficientBalance error, got: {:?}",
        result
    );
}
