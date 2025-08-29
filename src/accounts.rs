use std::collections::{BTreeMap, HashSet, btree_map};

use crate::{
    FixedPoint,
    input::{Input, TransactionType},
};

pub struct AccountStorage {
    accounts: BTreeMap<u16, Account>,
    used_txids: HashSet<u32>,
}

impl AccountStorage {
    /// create a new account storage
    pub fn new() -> Self {
        Self {
            accounts: BTreeMap::new(),
            used_txids: HashSet::new(),
        }
    }

    /// get client entry
    pub fn entry(&mut self, client: u16) -> btree_map::Entry<'_, u16, Account> {
        self.accounts.entry(client)
    }

    /// Get a reference to the account storage's accounts.
    pub fn accounts(&self) -> &BTreeMap<u16, Account> {
        &self.accounts
    }

    pub fn handle_transaction(&mut self, input: Input) -> Result<(), TransactionError> {
        if input.valid() {
            match input.r#type() {
                // safeguard agains duplicate transaction IDs by checking
                // if any previous transactions has used it
                TransactionType::Deposit | TransactionType::Withdrawal => {
                    if self.used_txids.contains(&input.tx()) {
                        return Err(TransactionError::DuplicateTxId);
                    }
                    // we store the txid since the input is both valid, has not been used before
                    // This is based upon the assumption that a transaction that fails,
                    // still was valid
                    self.used_txids.insert(input.tx());
                }
                _ => {
                    // The other types of transactions should act upon existing txids, but also on
                    // the specific account, thus we check that per account
                }
            }
            let account = self.entry(input.client()).or_insert(Account::new());

            // By consuming the input, we are safeguarding that we cannot use the input twice by mistake
            account.handle_transaction(input)?;
            Ok(())
        } else {
            Err(TransactionError::MalformedInput)
        }
    }
}

#[derive(Debug)]
pub enum TransactionError {
    /// The transaction Input was not incorrectly formed and thus should fail
    MalformedInput,
    /// There was not enough funds on the account to  handle the requested transaction
    NotEnoughAvailableFunds,
    /// The Transaction ID could not be found
    MissingTxId,
    /// The transaction has already been handled
    DuplicateTxId,
    /// Account has been locked, and thus no transaction should be valid
    AccountLocked,
    /// The transaction was not valid for some reason
    InvalidTx,
    /// The transactio ID to dispute was invalid for some reason
    InvalidTxForDispute,
    /// The TxId for the dispute was missing
    MissingDisputeTx,
    /// The Dispute has already been started
    DisputeAlreadyExist,
    /// The Dispute has already been resolved one way or another
    DisputeAlreadyHandled,
}

#[derive(PartialEq, Eq)]
pub enum DisputeState {
    Started,
    Reimbursed,
    Resolved,
}

impl DisputeState {
    fn new() -> Self {
        Self::Started
    }
}

pub struct Account {
    /// amount of usable funds for withdrawal, trading, etc
    available: FixedPoint,

    /// amount of held funds for dispute
    held: FixedPoint,

    /// if the account is locked or not
    locked: bool,

    /// Just store an entire history of each transaction performed
    tx_history: BTreeMap<u32, Input>,

    /// disputes
    disputes: BTreeMap<u32, DisputeState>,
}

impl<'a> Account {
    /// Generates a new empty Account
    pub fn new() -> Self {
        Account {
            available: FixedPoint::from_f64(0.0),
            held: FixedPoint::from_f64(0.0),
            locked: false,
            disputes: BTreeMap::new(),
            tx_history: BTreeMap::new(),
        }
    }
    /// available
    pub fn available(&self) -> FixedPoint {
        self.available
    }

    pub fn contains_txid(&self, txid: u32) -> bool {
        self.tx_history.contains_key(&txid)
    }

    /// Get the account's held.
    pub fn held(&self) -> FixedPoint {
        self.held
    }
    pub fn total(&self) -> FixedPoint {
        self.held + self.available
    }

    fn lock(&mut self) {
        self.locked = true;
    }

    /// Handle a transaction request on this account
    pub fn handle_transaction(&mut self, transaction: Input) -> Result<(), TransactionError> {
        if !transaction.valid() {
            return Err(TransactionError::InvalidTx);
        }
        if self.locked {
            // This is probably a much more complex case, since an account probably can have multiple
            // active disputes. But I also feel like trying to handle this without careful consideration
            // could be quite exploitable, which is unwanted. So I'll play it safe here, and just not handle more transactions
            // after a chargeback has occured
            return Err(TransactionError::AccountLocked);
        }

        let tx_res = match transaction.r#type() {
            TransactionType::Deposit => {
                // Safe because of the validity check on the transaction
                let amount = transaction.amount_as_fp().unwrap();
                self.deposit(amount);

                self.tx_history.insert(transaction.tx(), transaction);
                Ok(())
            }
            TransactionType::Withdrawal => {
                // Safe because of the validity check on the transaction
                let amount = transaction.amount_as_fp().unwrap();
                self.withdraw(amount)
            }
            TransactionType::Dispute => {
                // we need to look back into all of the history related to this client ( and this client only ),
                // to validate wheter the TX exists, and then we need to hold the amount found in that tx
                self.dispute(transaction.tx())
            }
            TransactionType::Resolve => {
                // We shall unlock the held funds, if the held funds exist ofcourse
                // If the held funds are already spent, for example by a withdrawal, then a dispute
                self.resolve(transaction.tx())
            }
            TransactionType::Chargeback => self.chargeback(transaction.tx()),
        };

        tx_res
    }

    fn deposit(&mut self, amount: FixedPoint) {
        self.available += amount;
    }

    fn withdraw(&mut self, amount: FixedPoint) -> Result<(), TransactionError> {
        if self.locked() {
            return Err(TransactionError::AccountLocked);
        }
        if self.available >= amount {
            self.available -= amount;
            Ok(())
        } else {
            Err(TransactionError::NotEnoughAvailableFunds)
        }
    }

    fn chargeback(&mut self, tx: u32) -> Result<(), TransactionError> {
        let input = self
            .tx_history
            .get(&tx)
            .ok_or(TransactionError::MissingTxId)?;

        let dispute = self
            .disputes
            .get_mut(&tx)
            .ok_or(TransactionError::MissingDisputeTx)?;

        // println!("checking dispute state input {:?}", input);
        if *dispute == DisputeState::Started {
            // println!("dispute has started");
            if let Some(amount) = input.amount_as_fp() {
                // println!("the tx in question has an amount");
                if self.held <= amount {
                    println!("the held amount covers the dispute reimbursement");
                    self.held -= amount;
                }
            }
            *dispute = DisputeState::Reimbursed;
            self.lock();
            Ok(())
        } else {
            Err(TransactionError::DisputeAlreadyHandled)
        }
    }

    fn resolve(&mut self, tx: u32) -> Result<(), TransactionError> {
        let input = self
            .tx_history
            .get(&tx)
            .ok_or(TransactionError::MissingTxId)?;

        // fetch the the tx under dispute, apply the reverse if state is disputed
        let dispute = self
            .disputes
            .get_mut(&tx)
            .ok_or(TransactionError::MissingDisputeTx)?;

        if *dispute == DisputeState::Started {
            if let Some(amount) = input.amount_as_fp() {
                let heldres = self.held - amount;
                if heldres < FixedPoint::from_f64(0.0) {
                    eprintln!(
                        "resolved a dispute resulting in negative held amount for TX: {}",
                        tx
                    );
                }
                self.held = heldres;
                self.available += amount;
                *dispute = DisputeState::Resolved;
                Ok(())
            } else {
                Err(TransactionError::InvalidTx)
            }
        } else {
            Err(TransactionError::DisputeAlreadyHandled)
        }
    }

    fn dispute(&mut self, tx: u32) -> Result<(), TransactionError> {
        // Fetch the tx that is to be disputed
        let input = self
            .tx_history
            .get(&tx)
            .ok_or(TransactionError::MissingTxId)?;

        match input.r#type() {
            TransactionType::Deposit => {
                if self.disputes.contains_key(&tx) {
                    Err(TransactionError::DisputeAlreadyExist)
                } else {
                    let amount = input
                        .amount_as_fp()
                        .ok_or(TransactionError::InvalidTxForDispute)?;

                    // store the tx under dispute, unless already handled
                    // hold the funds related in the dispute
                    self.disputes.insert(tx, DisputeState::new());
                    self.available -= amount;
                    self.held += amount;
                    Ok(())
                }
            }
            _ => Err(TransactionError::InvalidTxForDispute),
        }
    }

    /// Get the account's locked status
    pub fn locked(&self) -> bool {
        self.locked
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// Depositing into an empty account increases the available balance.
    fn test_account_deposit() {
        let mut account = Account::new();

        let transaction = Input::new(TransactionType::Deposit, 1, 1, Some(55.1234));
        let res = account.handle_transaction(transaction);
        assert!(res.is_ok(), "Deposit failed: {:?}", res);

        assert_eq!(55.1234, account.available());
        assert_eq!(55.1234, account.total());
    }

    #[test]
    /// Withdrawing more than the available balance should fail and leave balance unchanged.
    fn test_account_withdraw_too_much() {
        let mut account = Account::new();

        // Start with a deposit
        let deposit = Input::new(TransactionType::Deposit, 1, 1, Some(55.1234));
        account.handle_transaction(deposit).unwrap();

        // Attempt to overdraw
        let withdraw = Input::new(TransactionType::Withdrawal, 1, 2, Some(56.1234));
        let res = account.handle_transaction(withdraw);
        assert!(res.is_err(), "Expected withdrawal to fail");

        // Balance should remain unchanged
        assert_eq!(55.1234, account.available());
        assert_eq!(55.1234, account.total());
    }

    #[test]
    /// Withdrawing a valid small amount should succeed and reduce the balance.
    fn test_account_withdraw_partial() {
        let mut account = Account::new();

        // Start with a deposit
        let deposit = Input::new(TransactionType::Deposit, 1, 1, Some(55.1234));
        account.handle_transaction(deposit).unwrap();

        // Withdraw a small amount
        let withdraw = Input::new(TransactionType::Withdrawal, 1, 2, Some(0.1234));
        let res = account.handle_transaction(withdraw);
        assert!(res.is_ok(), "Partial withdrawal failed: {:?}", res);

        // Balance should decrease
        assert_eq!(55.0, account.available());
        assert_eq!(55.0, account.total());
    }

    #[test]
    /// Testing that chargebacks after a withdrawal of partial funds still succeeds
    /// the assumption here is that a chargeback is not something that we can control,
    /// but something that someone else is forcing upon us
    fn test_account_chargeback_after_withdrawal() {
        let mut account = Account::new();

        // Start with a deposit
        let deposit = Input::new(TransactionType::Deposit, 1, 1, Some(55.1234));
        account.handle_transaction(deposit).unwrap();

        // Withdraw a small amount
        let withdraw = Input::new(TransactionType::Withdrawal, 1, 2, Some(0.1234));
        let res = account.handle_transaction(withdraw);
        assert!(res.is_ok(), "Partial withdrawal failed: {:?}", res);

        // Balance should decrease
        assert_eq!(55.0, account.available());
        assert_eq!(55.0, account.total());

        let dispute = Input::new(TransactionType::Dispute, 1, 1, None);
        let res = account.handle_transaction(dispute);
        assert!(
            res.is_ok(),
            "dispute should fail since the funds are no longer there {:?}",
            res
        );
        // Balance should stay the same
        assert_eq!(-0.1234, account.available());
        assert_eq!(55., account.total());

        let chargeback = Input::new(TransactionType::Chargeback, 1, 1, None);
        let res = account.handle_transaction(chargeback);
        assert!(
            res.is_ok(),
            "dispute should fail since the funds are no longer there {:?}",
            res
        );
        // Balance should stay the same
        assert_eq!(-0.1234, account.available());
        assert_eq!(-0.1234, account.total());
        assert_eq!(0.0, account.held());
        assert_eq!(true, account.locked());
    }

    #[test]
    /// Withdrawing the full remaining balance should succeed and leave account at zero.
    fn test_account_withdraw_all() {
        let mut account = Account::new();

        // Start with a deposit
        let deposit = Input::new(TransactionType::Deposit, 1, 1, Some(55.0));
        account.handle_transaction(deposit).unwrap();

        // Withdraw everything
        let withdraw = Input::new(TransactionType::Withdrawal, 1, 2, Some(55.0));
        let res = account.handle_transaction(withdraw);
        assert!(res.is_ok(), "Full withdrawal failed: {:?}", res);

        // Account should be empty
        assert_eq!(0.0, account.available());
        assert_eq!(0.0, account.total());
    }

    #[test]
    /// tests that withdrawing not held funds still works even though an active dispute is underway
    fn account_deposited_dispute() {
        let mut account = Account::new();

        let transaction = Input::new(TransactionType::Deposit, 1, 1, Some(50.0));
        let res = account.handle_transaction(transaction);
        if let Err(e) = res {
            assert!(true, "{:?}", e);
        }

        let transaction = Input::new(TransactionType::Deposit, 1, 2, Some(5.1234));
        let res = account.handle_transaction(transaction);
        if let Err(e) = res {
            assert!(true, "{:?}", e);
        }
        // Withdrawing to much should fail
        assert_eq!(55.1234, account.available());

        // Withdrawing to much should fail
        let transaction = Input::new(TransactionType::Dispute, 1, 1, None);
        let res = account.handle_transaction(transaction);
        if let Err(e) = res {
            assert!(true, "{:?}", e);
        }
        assert_eq!(55.1234, account.total());
        assert_eq!(5.1234, account.available());
        assert_eq!(50.0, account.held());

        // Withdrawing a small amount should work, and in this case leave exactly 5.0000 left
        let transaction = Input::new(TransactionType::Withdrawal, 1, 3, Some(0.1234));
        let res = account.handle_transaction(transaction);
        if let Err(e) = res {
            assert!(true, "{:?}", e);
        }
        assert_eq!(5.0, account.available());
        assert_eq!(50.0, account.held());
        assert_eq!(55.0, account.total());
    }

    #[test]
    /// Tests the dispute and chargeback
    /// by depositing into a clients account, then
    /// disputing that deposit and returning the deposited amount
    fn account_dispute_chargeback() {
        let mut account = Account::new();

        let deposit = Input::new(TransactionType::Deposit, 1, 1, Some(50.0));
        let res = account.handle_transaction(deposit);
        if let Err(e) = res {
            assert!(true, "{:?}", e);
        }

        let dispute = Input::new(TransactionType::Dispute, 1, 1, None);
        let res = account.handle_transaction(dispute);
        if let Err(e) = res {
            assert!(true, "{:?}", e);
        }
        assert_eq!(0.0, account.available());
        assert_eq!(50.0, account.held());
        assert_eq!(50.0, account.total());
        assert_eq!(false, account.locked(), "account locked state was wrong");

        let chargeback = Input::new(TransactionType::Chargeback, 1, 1, None);
        let res = account.handle_transaction(chargeback);
        if let Err(e) = res {
            assert!(true, "{:?}", e);
        }
        assert_eq!(0.0, account.held(), "held amount was wrong");
        assert_eq!(0.0, account.available(), "available amount was wrong");
        assert_eq!(0.0, account.total(), "total amount was wrong");
        assert_eq!(true, account.locked(), "account locked state was wrong");
    }

    #[test]
    fn account_dispute_resolve() {
        let mut account = Account::new();

        // Deposit funds into the account
        let deposit = Input::new(TransactionType::Deposit, 1, 1, Some(50.0));
        let res = account.handle_transaction(deposit);
        assert!(res.is_ok(), "Deposit failed: {:?}", res);

        // Dispute the deposit: should move funds to `held`
        let dispute = Input::new(TransactionType::Dispute, 1, 1, None);
        let res = account.handle_transaction(dispute);
        assert!(res.is_ok(), "Dispute failed: {:?}", res);

        assert_eq!(0.0, account.available());
        assert_eq!(50.0, account.held());
        assert_eq!(50.0, account.total());
        assert!(
            !account.locked(),
            "Account should not be locked after a dispute"
        );

        // Resolve the dispute: should move funds back to `available`
        let resolve = Input::new(TransactionType::Resolve, 1, 1, None);
        let res = account.handle_transaction(resolve);
        assert!(res.is_ok(), "Resolve failed: {:?}", res);

        assert_eq!(
            50.0,
            account.available(),
            "Available amount was not restored"
        );
        assert_eq!(
            0.0,
            account.held(),
            "Held amount should be cleared after resolve"
        );
        assert_eq!(
            50.0,
            account.total(),
            "Total amount should remain unchanged"
        );
        assert!(
            !account.locked(),
            "Account should be not be locked after resolve"
        );
    }

    #[test]
    /// Depositing using a previously used TXID should fail to deposit.
    fn test_duplicate_transaction_same_client() {
        let mut accounts = AccountStorage::new();

        let transaction = Input::new(TransactionType::Deposit, 1, 1234, Some(55.1234));
        let res = accounts.handle_transaction(transaction);
        assert!(res.is_ok(), "Deposit failed: {:?}", res);

        let transaction = Input::new(TransactionType::Deposit, 1, 1234, Some(55.1234));
        let res = accounts.handle_transaction(transaction);
        assert!(res.is_err(), "Deposit failed: {:?}", res);

        assert_eq!(55.1234, accounts.accounts.get(&1).unwrap().available());
        assert_eq!(55.1234, accounts.accounts.get(&1).unwrap().total());
    }

    #[test]
    /// Depositing into an different clients account using a previously used TXID should not be a valid transaction
    fn test_duplicate_transaction_different_clients() {
        let mut accounts = AccountStorage::new();

        let transaction = Input::new(TransactionType::Deposit, 1, 1234, Some(55.1234));
        let res = accounts.handle_transaction(transaction);
        assert!(
            res.is_ok(),
            "Deposit failed when it should succeed: {:?}",
            res
        );

        let transaction = Input::new(TransactionType::Deposit, 2, 1234, Some(55.1234));
        let res = accounts.handle_transaction(transaction);
        assert!(
            res.is_err(),
            "Deposit succeded when it should fail: {:?}",
            res
        );

        assert_eq!(55.1234, accounts.accounts.get(&1).unwrap().available());
        assert!(
            accounts.accounts.get(&2).is_none(),
            "Account 2 should not exist due to invalid input"
        );
    }

    #[test]
    /// Tests that it is not possible to withdraw money from an accound after a
    /// successfull chargeback, since the account should then be locked.
    fn cannot_withdraw_after_chargeback() {
        let mut account = Account::new();

        let transaction = Input::new(TransactionType::Deposit, 1, 1, Some(50.0));
        let res = account.handle_transaction(transaction);
        assert!(
            res.is_ok(),
            "Deposit failed when it should succeed: {:?}",
            res
        );

        let transaction = Input::new(TransactionType::Deposit, 1, 2, Some(0.1234));
        let res = account.handle_transaction(transaction);
        assert!(
            res.is_ok(),
            "Deposit failed when it should succeed: {:?}",
            res
        );

        let transaction = Input::new(TransactionType::Dispute, 1, 1, None);
        let res = account.handle_transaction(transaction);
        assert!(
            res.is_ok(),
            "Dispute failed when it should succeed: {:?}",
            res
        );

        let transaction = Input::new(TransactionType::Chargeback, 1, 1, None);
        let res = account.handle_transaction(transaction);

        assert!(res.is_ok(), "Chargeback shuld succeed");
        assert!(account.locked(), "account should be locked");

        let transaction = Input::new(TransactionType::Withdrawal, 1, 3, Some(0.1234));
        let res = account.handle_transaction(transaction);
        assert!(
            res.is_err(),
            "Withdrawal should not succeed since account should be locked"
        );
    }
}
