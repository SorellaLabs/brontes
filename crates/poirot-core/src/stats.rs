use crate::{errors::TraceParseError, format_color};
use tracing::{
    field::{Field, Visit},
    span::Attributes,
    Id, Subscriber, info,
};
use tracing_subscriber::{layer::Context, registry::LookupSpan, Layer};

pub struct ParserStatsLayer;

#[derive(Debug)]
pub struct ParserStats {
    pub total_tx: usize,
    pub total_traces: usize,
    pub successful_parses: usize,
    pub empty_input_errors: usize,
    pub etherscan_errors: usize,
    pub abi_parse_errors: usize,
    pub invalid_function_selector_errors: usize,
    pub abi_decoding_failed_errors: usize,
    pub trace_missing_errors: usize,
}

impl Default for ParserStats {
    fn default() -> Self {
        Self { total_tx: Default::default(), total_traces: Default::default(), successful_parses: Default::default(), empty_input_errors: Default::default(), etherscan_errors: Default::default(), abi_parse_errors: Default::default(), invalid_function_selector_errors: Default::default(), abi_decoding_failed_errors: Default::default(), trace_missing_errors: Default::default() }
    }
}

impl ParserStats {
    pub fn print_stats(&self) {
        info!("{}", format_color("Total Transactions", self.total_tx));
        println!(
            "
Total Transactions: {} 
Total Traces: {}
Successful Parses: {}
Empty Input Errors: {}
Etherscan Errors: {}
ABI Parse Errors: {}
Invalid Function Selector Errors: {}
ABI Decoding Failed Errors: {}
Trace Missing Errors: {}\n",
            self.total_tx,
            self.total_traces,
            self.successful_parses,
            self.empty_input_errors,
            self.etherscan_errors,
            self.abi_parse_errors,
            self.invalid_function_selector_errors,
            self.abi_decoding_failed_errors,
            self.trace_missing_errors
        );
    }
}

impl<S> Layer<S> for ParserStatsLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(&self, _attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let span = ctx.span(id).unwrap();

        span.extensions_mut().insert(ParserStats::default());
    }

    fn on_event(&self, event: &tracing::Event<'_>, ctx: tracing_subscriber::layer::Context<'_, S>) {
        if let Some(id) = ctx.current_span().id() {
            let span = ctx.span(id).unwrap();
            if let Some(ext) = span.extensions_mut().get_mut::<ParserStats>() {
                if event.metadata().target().contains("Finished Parsing Block") {
                    ext.print_stats();
                } else {
                    event.record(&mut *ext);
                }
            };
        }
    }
}

impl Visit for ParserStats {
    /// will implement incrementing counters for tx/block traces
    /// tbd
    /// find a better way to do this
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        let value_str = format!("{:?}", value);
        if value_str.contains("TraceMissing") {
            self.trace_missing_errors += 1;
        } else if value_str.contains("EmptyInput") {
            self.empty_input_errors += 1;
        } else if value_str.contains("EtherscanError") {
            self.etherscan_errors += 1;
        } else if value_str.contains("AbiParseError") {
            self.abi_parse_errors += 1;
        } else if value_str.contains("InvalidFunctionSelector") {
            self.abi_parse_errors += 1;
        } else if value_str.contains("AbiDecodingFailed") {
            self.abi_decoding_failed_errors += 1;
        } else if value_str.contains("Successfully Parsed Transaction") {
            self.total_tx += 1;
        } else if value_str.contains("Successfully Parsed Trace") {
            self.total_traces += 1;
        } else if value_str.contains("Finished Parsing Block") {
            self.print_stats();
        } else {
            //println!("{}", value_str);
        }
    }

    fn record_error(&mut self, _field: &Field, value: &(dyn std::error::Error + 'static)) {
        println!("hERE ERROR DASDSAD ");
        if let Some(error) = value.downcast_ref::<TraceParseError>() {
            match error {
                TraceParseError::TraceMissing => self.trace_missing_errors += 1,
                TraceParseError::EmptyInput(_) => self.empty_input_errors += 1,
                TraceParseError::EtherscanError(_) => self.etherscan_errors += 1,
                TraceParseError::AbiParseError(_) => self.abi_parse_errors += 1,
                TraceParseError::InvalidFunctionSelector(_) => self.abi_parse_errors += 1,
                TraceParseError::AbiDecodingFailed(_) => self.abi_decoding_failed_errors += 1,
            }
        }
    }
}
