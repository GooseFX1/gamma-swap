// From the kamnio IDL we have removed the refreshReservesBatch instruction, as the lifetime was not being used in that and it was failing to generate the CPI crate.
// Here is the removed instruction:
//  {
// "name": "refreshReservesBatch",
// "accounts": [],
// "args": [
//   {
//     "name": "skipPriceUpdates",
//     "type": "bool"
//   }
// ]
// }
// This should not cause any issues for making the cpi calls for other instructions.

declare_id!("KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD");

anchor_gen::generate_cpi_crate!("src/kamino.json");

pub struct Kamino;

impl Id for Kamino {
    fn id() -> Pubkey {
        ID
    }
}
