use candid::{Nat, Principal};
use ic_cdk::api::call::call;
use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
use ic_stable_structures::{DefaultMemoryImpl, StableBTreeMap, StableCell};
use lazy_static::lazy_static;
use sha2::{Digest, Sha256};
use std::cell::RefCell;

lazy_static! {
    pub static ref TRANSFER_FEE: Nat = Nat::from(10_000u64);
}

pub mod types;

pub use types::{Account, Deposit, DepositError, TransferArg, TransferError};

type Memory = VirtualMemory<DefaultMemoryImpl>;

thread_local! {
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> =
        RefCell::new(MemoryManager::init(DefaultMemoryImpl::default()));

    static TOKEN_ID: RefCell<StableCell<Principal, Memory>> = RefCell::new(
        StableCell::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(0))),
            Principal::anonymous()
        ).expect("Failed to initialize StableCell")
    );

    static DEPOSIT_COUNTER: RefCell<StableCell<u64, Memory>> = RefCell::new(
        StableCell::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(1))),
            0u64
        ).expect("Failed to initialize StableCell")
    );

    static DEPOSITS: RefCell<StableBTreeMap<u64, Deposit, Memory>> = RefCell::new(
        StableBTreeMap::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(2))),
        )
    );
}

#[ic_cdk::query]
pub fn get_deposit_subaccount(user: Principal, timelock: u64) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(user.as_slice());
    hasher.update(timelock.to_be_bytes());
    hasher.finalize().into()
}

#[ic_cdk::init]
pub fn init(token_id: Principal) {
    TOKEN_ID.with(|cell| {
        cell.borrow_mut().set(token_id).ok();
    });
}

#[ic_cdk::update]
pub async fn deposit(user: Principal, timelock: u64) -> Result<(), types::DepositError> {
    if timelock == 0 {
        return Err(types::DepositError::InvalidTimelock);
    }

    let ledger_principal = TOKEN_ID
        .with(|cell| {
            let stored = cell.borrow().get().clone();
            Some(stored)
        })
        .ok_or(types::DepositError::LedgerNotSet)?;

    let subaccount = get_deposit_subaccount(user, timelock);
    let from_account = types::Account {
        owner: ic_cdk::api::id(),
        subaccount: Some(subaccount.to_vec()),
    };

    let balance_result: Result<(Nat,), _> =
        call(ledger_principal, "icrc1_balance_of", (from_account,)).await;

    let balance = match balance_result {
        Ok((balance,)) => balance,
        Err(_) => return Err(types::DepositError::LedgerCallFailed),
    };

    let transfer_amount = if balance.0 > TRANSFER_FEE.0 {
        Nat::from(&balance.0 - &TRANSFER_FEE.0)
    } else {
        return Err(types::DepositError::InsufficientBalance);
    };

    let transfer_args = (types::TransferArg {
        from_subaccount: Some(subaccount.to_vec()),
        to: types::Account {
            owner: ic_cdk::api::id(),
            subaccount: None,
        },
        amount: transfer_amount.clone(),
        fee: Some(TRANSFER_FEE.clone()),
        memo: None,
        created_at_time: None,
    },);

    let transfer_result: Result<(Result<Nat, types::TransferError>,), _> =
        call(ledger_principal, "icrc1_transfer", transfer_args).await;

    if transfer_result.is_err() || transfer_result.as_ref().unwrap().0.is_err() {
        return Err(types::DepositError::TransferFailed);
    }

    let current_time = ic_cdk::api::time();
    let unlock_time = current_time + (timelock * 1_000_000_000);
    let deposit_id = DEPOSIT_COUNTER.with(|counter| {
        let current = counter.borrow().get().clone();
        let new_counter = current + 1;
        counter.borrow_mut().set(new_counter).ok();
        current
    });
    let deposit = types::Deposit {
        unlocktime: unlock_time,
        principal: user,
        amount: transfer_amount.clone(),
    };

    DEPOSITS.with(|deposits| {
        deposits.borrow_mut().insert(deposit_id, deposit);
    });
    Ok(())
}

ic_cdk::export_candid!();
