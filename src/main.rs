mod accounts;
mod input;
mod output;
mod simple_fp;

use simple_fp::FixedPoint;
// mod transaction;

fn main() {
    let filename = std::env::args()
        .into_iter()
        .nth(1)
        .expect("Expected file name as argument");

    let mut csv_reader = input::create_input_deserializer(&filename);

    // initialize a new account database
    let mut accounts = accounts::AccountStorage::new();

    let csv_iter = csv_reader.deserialize::<input::Input>();
    // every entry is a transaction and we just ignore any faulty parsed inputs
    for transaction in csv_iter.filter_map(|row| row.ok()) {
        if let Err(_e) = accounts.handle_transaction(transaction) {
            // here one would normally log any error to transactions
        }
    }

    output::print_from_accounts(accounts);
}
