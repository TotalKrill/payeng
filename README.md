# Toy Payment Engine

A toy payment engine


# Implmenentation

During implementation of this paymenent engine, there was some confusions about the rules of chargebacks and such.

The assumption made is that a chargeback is something mandated, and not something that can be choosen to do.
So that can result in negative values on the account if there has been a chargeback after a withdrawal has been made...

the other way would have been to automatically refuse disputes on transactions that have had withdrawals after the disputed
transaction that if chargebacked would make the balance go below zero. 

This payment engine does not allow withdrawal on locked accounts, but it allows for deposits on locked accounts, because its
unclear what happened that allowed a deposit on a locked account, but at least with the money in our control, it can be
verified better

The input is read in an iterator over a csv reader, so that the entire input is not held in ram at once, but handled one-by-one. The
solution still consumes quite a bit of ram since the transactions are held in ram under each account, so that disputes and such can be handled and checked.
It also checks for duplicate txid globally, to invalidate transactions that are reusing a txid. I chose to handle failed transactions (withdrawing to much)
as a valid txid, but maybe only successfull transactions should be counted towards the used txid's.

Some testdata was generated to see that it would handle larger datasets, its not very complete data, but at least it finished quite quickly

The fixed point implementation was done to avoid having any kind of rounding errors when doing the math.

The getters on the input struct is so that one should not be able to accidentally manipulate the input while handling it.

Generally I prefer to use type-safety for meanings of certain values, such as TxId(u32), and ClientId(u16), especially in larger
systems where there can be many of the same type, like different UUIDS or string identifiers of different types so that when going back to code, its easy to see which ID is expected
where. But in this case, all the IDs were separate types anyway...

AI usage has been quite low, mostly for generating the tests, and for small functions or methods that do specific usecases.

hope any scrutinizing eyes enjoy it!
