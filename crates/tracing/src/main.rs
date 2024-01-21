/*
use std::{fmt::Display, time::{SystemTime, UNIX_EPOCH}};

use tracing::{
    event,
    field::{AsField, Visit},
    info, instrument,
    span::Attributes,
    Id, Level, Subscriber,
};
use tracing_subscriber::{
    field::debug, layer::Context, prelude::__tracing_subscriber_SubscriberExt,
    registry::LookupSpan, util::SubscriberInitExt, Layer,
};

#[derive(Debug)]
pub enum CounterEnum {
    CountA(u64),
    CountB,
}

impl Display for CounterEnum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CounterEnum::CountA(_) => println!("Counted A"),
            CounterEnum::CountB => println!("Counted B"),
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct Counter {
    count_a: u64,
    count_b: u64,
}

impl Display for Counter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        println!("{:?}", &self);
        Ok(())
    }
}

impl<S> Layer<S> for Counter
where
    S: Subscriber,
    S: for<'lookup> LookupSpan<'lookup>,
{
    fn on_event(&self, event: &tracing::Event<'_>, ctx: tracing_subscriber::layer::Context<'_, S>) {
        let f = event.fields().next().take().unwrap();
        let k = f.callsite();
        if let Some(id) = ctx.current_span().id() {
            let span = ctx.span(id).unwrap();
            println!("here event name: {:?}\n lol", span.fields());
            if let Some(ext) = span.extensions_mut().get_mut::<Counter>() {
                event.record(&mut *ext);
            };
        }
    }

    fn on_new_span(&self, _attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let span = ctx.span(id).unwrap();

        span.extensions_mut().insert(Counter { count_a: 0, count_b: 0 });
    }

    fn on_close(&self, id: Id, ctx: Context<'_, S>) {
        let span = ctx.span(&id).unwrap();

        let binding = span.extensions();
        let s = binding.get::<Counter>().unwrap();

        //println!("span{:?} -- a: {} b: {}", span.metadata().fields().into_iter(), s.count_a, s.count_b,);
    }
}

impl Visit for Counter {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "count_type" {
            match format!("{:?}", value).as_str() {
                "CountA" => self.count_a += 1,
                "CountB" => self.count_b += 1,
                _ =>() //println!("{}", format!("{:?}", value).as_str()),
            }
        }
    }
}

#[instrument(name = "test_span", fields(blocks))]
pub fn count() {
    count2();
    for i in 0..10 {
        let count_type = if i % 2 == 0 { CounterEnum::CountA(1) } else { CounterEnum::CountB };
        info!(%count_type);
    }
}

pub fn count2() {
    for i in 0..10 {
        let count_type = if i % 3 == 0 { CounterEnum::CountA(1) } else { CounterEnum::CountB };
        let count = Counter { count_a: 0, count_b: 0 };
        info!(target:"cc", ?count);
    }
}

fn main() {
    tracing_subscriber::registry::Registry::default()
        .with(Counter { count_a: 0, count_b: 0 })
        .with(tracing_subscriber::fmt::layer())
        .init();
    //println!("{:?}", SystemTime::now().duration_since(UNIX_EPOCH));
    count();
}
*/

fn main() {}
