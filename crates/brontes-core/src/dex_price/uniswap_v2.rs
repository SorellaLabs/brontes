struct V2Pricing;

impl DexPrice for V2Pricing {
    fn get_price(
        &self,
        provider: &Provider<Http<reqwest::Client>>,
        address: Address,
        zto: bool,
        state_diff: StateDiff,
    ) -> Pin<Box<dyn Future<Output = (Rational, Rational)> + Send + Sync>> {
        Box::pin(async { todo!() })
    }
}
