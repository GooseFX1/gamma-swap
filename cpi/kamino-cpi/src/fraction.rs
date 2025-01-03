use fixed::traits::FromFixed;
pub use fixed::types::U68F60 as Fraction;

pub trait FractionExtra {
    fn to_floor<Dst: FromFixed>(&self) -> Dst;
    fn to_ceil<Dst: FromFixed>(&self) -> Dst;
}

impl FractionExtra for Fraction {
    #[inline]
    fn to_floor<Dst: FromFixed>(&self) -> Dst {
        self.floor().to_num()
    }

    #[inline]
    fn to_ceil<Dst: FromFixed>(&self) -> Dst {
        self.ceil().to_num()
    }
}
