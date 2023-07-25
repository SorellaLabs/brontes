use std::collections::BTreeMap;

use crate::{format_color, errors::TraceParseError};
use tracing::{
    field::{Field, Visit},
    span::Attributes,
    Id, Subscriber, info, Level,
};
use tracing_subscriber::{layer::Context, registry::LookupSpan, Layer, EnvFilter, FmtSubscriber};

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


struct ParserStatsVisitor(BTreeMap<&'static str, u64>);

impl ParserStatsVisitor {
    pub fn new() -> Self {
        let mut map = BTreeMap::new();
        map.insert("TraceMissing", 0);
        map.insert("EmptyInput", 0);
        map.insert("EtherscanError", 0);
        map.insert("AbiParseError", 0);
        map.insert("InvalidFunctionSelector", 0);
        map.insert("AbiDecodingFailed", 0);
        ParserStatsVisitor(map)
    }
}


impl Default for ParserStats {
    fn default() -> Self {
        Self { total_tx: Default::default(), total_traces: Default::default(), successful_parses: Default::default(), empty_input_errors: Default::default(), etherscan_errors: Default::default(), abi_parse_errors: Default::default(), invalid_function_selector_errors: Default::default(), abi_decoding_failed_errors: Default::default(), trace_missing_errors: Default::default() }
    }
}

impl ParserStats {
    /// Since we are calling this from another layer that doesn't implement outputing to stdout
    /// We can initiate a fmt layer to output the stats as such
    pub fn print_stats(&self) {
        tracing::subscriber::with_default(
            FmtSubscriber::builder()
                .with_env_filter(EnvFilter::builder().with_default_directive(Level::INFO.into()).from_env_lossy())
                .finish(), 
            || {
                //println!(); // for separation between stats
                info!("{}", format_color("Total Transactions", self.total_tx, false));
                info!("{}", format_color("Total Traces", self.total_traces, false));
                info!("{}", format_color("Successful Parses", self.successful_parses, false));
                info!("{}", format_color("Empty Input Errors", self.empty_input_errors, true));
                info!("{}", format_color("Etherscan Errors", self.etherscan_errors, true));
                info!("{}", format_color("ABI Parse Errors", self.abi_parse_errors, true));
                info!("{}", format_color("Invalid Function Selector Errors", self.invalid_function_selector_errors, true));
                info!("{}", format_color("ABI Decoding Failed Errors", self.abi_decoding_failed_errors, true));
                info!("{}\n", format_color("Trace Missing Errors", self.trace_missing_errors, true));
            }
        );
    }
}

impl<S> Layer<S> for ParserStatsLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(&self, _attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let span = ctx.span(id).unwrap();

        span.extensions_mut().insert(ParserStatsVisitor::new());
    }

    fn on_event(&self, event: &tracing::Event<'_>, ctx: tracing_subscriber::layer::Context<'_, S>) {
        if let Some(id) = ctx.current_span().id() {
            let span = ctx.span(id).unwrap();
            if let Some(ext) = span.extensions_mut().get_mut::<ParserStatsVisitor>() {
                event.record(&mut *ext);
            };
        }
    }
}

impl Visit for ParserStatsVisitor {
    /// will implement incrementing counters for tx/block traces
    /// find a better way to do this
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        println!("RECORD DEBUG {:?}", field.name());
      /* 
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
            self.successful_parses += 1;
        } else if value_str.contains("Starting Trace") {
            self.total_traces += 1;
        } else if value_str.contains("Finished Parsing Block") {
            self.print_stats();
        }
        */
    }

    // tbd
    
    fn record_error(&mut self, _field: &Field, value: &(dyn std::error::Error + 'static)) {
        println!("RECORD ERROR");
        if let Some(error) = value.downcast_ref::<TraceParseError>() {
            match error {
                TraceParseError::TraceMissing => *self.0.get_mut("TraceMissing").unwrap() += 1,
                TraceParseError::EmptyInput(_) => *self.0.get_mut("EmptyInput").unwrap() += 1,
                TraceParseError::EtherscanError(_) => *self.0.get_mut("EtherscanError").unwrap() += 1,
                TraceParseError::AbiParseError(_) => *self.0.get_mut("AbiParseError").unwrap() += 1,
                TraceParseError::InvalidFunctionSelector(_) => *self.0.get_mut("InvalidFunctionSelector").unwrap() += 1,
                TraceParseError::AbiDecodingFailed(_) => *self.0.get_mut("AbiDecodingFailed").unwrap() += 1,
            }
        }
    }
    
}


