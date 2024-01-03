sol! (SushiSwapV2, "./abis/SushiSwapV2.json");
sol! (UniswapV2, "./abis/UniswapV2.json");
sol! (UniswapV3, "./abis/UniswapV3.json");
sol! (SushiSwapV3, "./abis/SushiSwapV3.json");
sol!(AaveV2, "./abis/AaveV2Pool.json")


#[allow(non_camel_case_types)]
pub enum StaticBindings {
   SushiSwapV2(SushiSwapV2_Enum),
   UniswapV2(UniswapV2_Enum),
   UniswapV3(UniswapV3_Enum),
   SushiSwapV3(SushiSwapV3_Enum),
   AaveV2(AaveV2_Enum),
}
impl StaticBindings {
 pub fn try_decode(&self, call_data: &[u8]) -> Result<StaticReturnBindings, alloy_sol_types::Error> {
     match self {
       StaticBindings::SushiSwapV2(_) => Ok(StaticReturnBindings::SushiSwapV2(SushiSwapV2_Enum::try_decode(call_data)?)),
       StaticBindings::UniswapV2(_) => Ok(StaticReturnBindings::UniswapV2(UniswapV2_Enum::try_decode(call_data)?)),
       StaticBindings::UniswapV3(_) => Ok(StaticReturnBindings::UniswapV3(UniswapV3_Enum::try_decode(call_data)?)),
       StaticBindings::SushiSwapV3(_) => Ok(StaticReturnBindings::SushiSwapV3(SushiSwapV3_Enum::try_decode(call_data)?)),
       StaticBindings::AaveV2(_) => Ok(StaticBindings::AaveV2(AaveV2_Enum::try_decode(call_data)?)),
}
 }
}


#[allow(non_camel_case_types)]
pub enum StaticReturnBindings {
   SushiSwapV2(SushiSwapV2::SushiSwapV2Calls),
   UniswapV2(UniswapV2::UniswapV2Calls),
   UniswapV3(UniswapV3::UniswapV3Calls),
   SushiSwapV3(SushiSwapV3::SushiSwapV3Calls),
   AaveV2(AaveV2::AaveV2Calls)
}

#[allow(non_camel_case_types)]
pub enum SushiSwapV2_Enum {
 None
}
impl_decode_sol!(SushiSwapV2_Enum, SushiSwapV2::SushiSwapV2Calls);



#[allow(non_camel_case_types)]
pub enum UniswapV2_Enum {
 None
}
impl_decode_sol!(UniswapV2_Enum, UniswapV2::UniswapV2Calls);



#[allow(non_camel_case_types)]
pub enum UniswapV3_Enum {
 None
}
impl_decode_sol!(UniswapV3_Enum, UniswapV3::UniswapV3Calls);



#[allow(non_camel_case_types)]
pub enum SushiSwapV3_Enum {
 None
}
impl_decode_sol!(SushiSwapV3_Enum, SushiSwapV3::SushiSwapV3Calls);


#[allow(non_camel_case_types)]
pub enum AaveV2_Enum { 
   None
}
impl_decode_sol(AaveV2_Enum, AaveV2::AaveV2Calls);