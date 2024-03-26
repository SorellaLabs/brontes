use std::marker::PhantomData;

use alloy_primitives::Address;
use clickhouse::Row;
use itertools::Itertools;
use malachite::{
    num::basic::traits::{One, Zero},
    Rational,
};
use rayon::iter::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};
use redefined::{Redefined, RedefinedConvert};
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{Deserialize, Serialize};

use super::cex::CexExchange;
use crate::{
    db::redefined_types::malachite::RationalRedefined,
    implement_table_value_codecs_with_zc,
    pair::{Pair, PairRedefined},
    FastHashMap, FastHashSet,
};

/// TODO: lets prob not set this to 100%
const BASE_EXECUTION_QUALITY: usize = 100;
/// The amount of excess volume a trade can do to be considered
/// as part of execution
const EXCESS_VOLUME_PCT: Rational = Rational::const_from_unsigneds(5, 100);

/// the calcuated price based off of trades with the estimated exchanges with
/// volume amount that where used to hedge
#[derive(Debug, Clone)]
pub struct ExchangePrice {
    // cex exchange with amount of volume executed on it
    pub exchanges: Vec<(CexExchange, Rational)>,
    pub price:     Rational,
}

type MakerTaker = (ExchangePrice, ExchangePrice);

type RedefinedTradeMapVec = Vec<(PairRedefined, Vec<CexTradesRedefined>)>;

// cex trades are sorted from lowest fill price to highest fill price
#[derive(Debug, Default, Clone, Row, PartialEq, Eq, Serialize, Deserialize)]
pub struct CexTradeMap(pub FastHashMap<CexExchange, FastHashMap<Pair, Vec<CexTrades>>>);
#[derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive, Redefined)]
#[redefined(CexTradeMap)]
#[redefined_attr(
    to_source = "CexTradeMap(self.map.into_iter().map(|(k,v)| \
                 (k,v.into_iter().collect::<FastHashMap<_,_>>())).collect::<FastHashMap<_,_>>().\
                 to_source())",
    from_source = "CexTradeMapRedefined::new(src.0)"
)]
pub struct CexTradeMapRedefined {
    pub map: Vec<(CexExchange, RedefinedTradeMapVec)>,
}

impl CexTradeMapRedefined {
    fn new(map: FastHashMap<CexExchange, FastHashMap<Pair, Vec<CexTrades>>>) -> Self {
        Self {
            map: map
                .into_iter()
                .map(|(exch, inner_map)| {
                    (
                        exch,
                        inner_map
                            .into_iter()
                            .map(|(a, b)| {
                                (
                                    PairRedefined::from_source(a),
                                    Vec::<CexTradesRedefined>::from_source(b),
                                )
                            })
                            .collect_vec(),
                    )
                })
                .collect::<Vec<_>>(),
        }
    }
}

implement_table_value_codecs_with_zc!(CexTradeMapRedefined);

type FoldVWAM = FastHashMap<Address, Vec<MakerTaker>>;

impl CexTradeMap {
    // Calculates VWAPs for the given pair across all provided exchanges - this
    // will assess trades across each exchange
    //
    // For non-intermediary dependant pairs the following process is followed:
    // - 1. Adjust each exchanges trade set by the assumed execution quality for the
    //   given pair on the exchange, we assess a larger percent of trades if
    //   execution quality is assumed to be lower
    // - 2. Exclude trades that have volume too large to be considered as potential
    //   hedging trades
    // - 3. Order all trades for each exchange by price
    // - 4. Finally we pick a vec of trades whose total volume is closest to the
    //   swap volume
    // - 5. Calculates VWAP for the chosen set of trades

    // For non-intermediary dependant pairs
    // - 1. Calculate VWAPs for all potential intermediary pairs (using above
    //   process)
    // -- Pair's with insufficient volume will be filtered out here which will
    // filter the route in the next step
    // - 2. Combines VWAP's to assess potential routes
    // - 3. Selects most profitable route and returns it as the Price
    // -- It should be noted here that this will not aggregate multiple possible
    // routes
    pub fn get_price(
        &self,
        exchanges: &[CexExchange],
        pair: &Pair,
        volume: &Rational,
        baskets: usize,
        quality: Option<FastHashMap<CexExchange, FastHashMap<Pair, usize>>>,
    ) -> Option<MakerTaker> {
        self.get_vwam_no_intermediary(exchanges, pair, volume, baskets, quality.as_ref())
            .or_else(|| {
                self.get_vwam_via_intermediary(exchanges, pair, volume, baskets, quality.as_ref())
            })
    }

    fn calculate_intermediary_addresses(
        &self,
        exchanges: &[CexExchange],
        pair: &Pair,
    ) -> FastHashSet<Address> {
        self.0
            .par_iter()
            .filter(|(k, _)| exchanges.contains(k))
            .flat_map(|(_, pairs)| {
                pairs
                    .keys()
                    .filter_map(|trade_pair| {
                        (trade_pair.0 == pair.0 && trade_pair.1 != pair.1)
                            .then_some(pair.1)
                            .or_else(|| {
                                (trade_pair.0 != pair.0 && trade_pair.1 == pair.1).then_some(pair.0)
                            })
                    })
                    .collect_vec()
            })
            .collect::<FastHashSet<_>>()
    }

    fn get_vwam_via_intermediary(
        &self,
        exchanges: &[CexExchange],
        pair: &Pair,
        volume: &Rational,
        baskets: usize,
        quality: Option<&FastHashMap<CexExchange, FastHashMap<Pair, usize>>>,
    ) -> Option<MakerTaker> {
        let fold_fn = |(mut pair0_vwam, mut pair1_vwam): (FoldVWAM, FoldVWAM), (iter_0, iter_1)| {
            for (k, v) in iter_0 {
                pair0_vwam.entry(k).or_insert(vec![]).extend(v);
            }
            for (k, v) in iter_1 {
                pair1_vwam.entry(k).or_insert(vec![]).extend(v);
            }

            (pair0_vwam, pair1_vwam)
        };

        // calculates vwam's and mutates iterator so quotes can be reasonably
        // compared. We calculate vwaps for pairs of all possible intermediary
        // paths and later exclude unlikely ones
        let (pair0_vwams, pair1_vwams) = self
            .calculate_intermediary_addresses(exchanges, pair)
            .into_par_iter()
            .filter_map(|intermediary| {
                let pair0 = Pair(pair.0, intermediary);
                let pair1 = Pair(intermediary, pair.1);
                Some((
                    (
                        intermediary,
                        self.get_vwam_no_intermediary(exchanges, &pair0, volume, baskets, quality)?,
                    ),
                    (
                        intermediary,
                        self.get_vwam_no_intermediary(exchanges, &pair1, volume, baskets, quality)?,
                    ),
                ))
            })
            .fold(
                || (FastHashMap::default(), FastHashMap::default()),
                |(mut pair0_vwam, mut pair1_vwam), ((iter0, prices0), (iter1, prices1))| {
                    pair0_vwam.entry(iter0).or_insert(vec![]).push(prices0);
                    pair1_vwam.entry(iter1).or_insert(vec![]).push(prices1);
                    (pair0_vwam, pair1_vwam)
                },
            )
            .reduce(|| (FastHashMap::default(), FastHashMap::default()), fold_fn);

        // calculates best possible cross_pair execution price
        calculate_cross_pair(pair0_vwams, pair1_vwams)
    }

    fn get_vwam_no_intermediary(
        &self,
        exchanges: &[CexExchange],
        pair: &Pair,
        volume: &Rational,
        baskets: usize,
        quality: Option<&FastHashMap<CexExchange, FastHashMap<Pair, usize>>>,
    ) -> Option<MakerTaker> {
        // Populate Map of Assumed Execution Quality by Exchange
        // - We're making the assumption that the stat arber isn't hitting *every* good
        //   markout for each pair on each exchange.
        // - Quality percent adjusts the total percent of "good" trades the arber is
        //   capturing for the relevant pair on a given exchange.
        let quality_pct = quality.map(|map| {
            map.iter()
                .map(|(k, v)| (*k, v.get(pair).copied().unwrap_or(BASE_EXECUTION_QUALITY)))
                .collect::<FastHashMap<_, _>>()
        });

        // Filter Exchange Trades Based On Volume
        // - This filters trades used to calculate the VWAM by excluding trades that
        //   have significantly more volume than the needed inventory offset
        // - The assumption here is the stat arber is trading just for this arb and
        //   isn't offsetting inventory for other purposes at the same time
        let max_vol_per_trade = volume + (volume * EXCESS_VOLUME_PCT);
        let trades = self
            .0
            .iter()
            .filter(|(e, _)| exchanges.contains(e))
            .filter_map(|(exchange, trades)| {
                Some((
                    *exchange,
                    trades.get(pair).map(|trades| {
                        trades
                            .iter()
                            .filter(|f| f.amount.le(&max_vol_per_trade))
                            .collect_vec()
                    })?,
                ))
            })
            .collect::<Vec<_>>();

        if trades.is_empty() {
            return None;
        }
        // Populate trade queue per exchange
        // - This utilizes the quality percent number to set the number of trades that
        //   will be assessed in picking a bucket to calculate the vwam with. A lower
        //   quality percent will cause us to examine more trades (go deeper into the
        //   vec) - resulting in a potentially worse price
        let trade_queue = PairTradeQueue::new(trades, quality_pct);

        self.get_most_accurate_basket(trade_queue, volume, baskets)
    }

    fn get_most_accurate_basket(
        &self,
        mut queue: PairTradeQueue<'_>,
        volume: &Rational,
        baskets: usize,
    ) -> Option<MakerTaker> {
        let mut trades = Vec::new();

        // multiply volume by baskets to assess more potential baskets of trades
        let volume_amount = volume * Rational::from(baskets);
        let mut cur_vol = Rational::ZERO;

        // Populates an ordered vec of trades from best to worst price
        while volume_amount.gt(&cur_vol) {
            let Some(next) = queue.next_best_trade() else {
                break;
            };
            // we do this due to the sheer amount of trades we have and to not have to copy.
            // all of this is safe
            cur_vol += &next.get().amount;

            trades.push(next);
        }
        // Groups trades into a set of iterators, first including all trades, then all
        // combinations of 2 trades, then all combinations of 3 trades, then all
        // combinations of 4 trades
        // - The assumption here is we don't frequently need to evaluate more than a set
        //   of 4 trades
        let trade_buckets_iterator = trades.iter().map(|t| vec![t]).chain(
            trades
                .iter()
                .combinations(2)
                .chain(trades.iter().combinations(3))
                .chain(trades.iter().combinations(4)),
        );
        // Gets the vec of trades that's closest to the volume of the stat arb swap
        // Will not return a vec that does not have enough volume to fill the arb
        let closest = closest(trade_buckets_iterator, volume)?;

        let mut vxp_maker = Rational::ZERO;
        let mut vxp_taker = Rational::ZERO;
        let mut trade_volume = Rational::ZERO;
        let mut exchange_with_vol = FastHashMap::default();

        // For the closest basket sum volume and volume weighted prices
        for trade in closest {
            let (m_fee, t_fee) = trade.get().exchange.fees();

            vxp_maker += (&trade.get().price * (Rational::ONE - m_fee)) * &trade.get().amount;
            vxp_taker += (&trade.get().price * (Rational::ONE - t_fee)) * &trade.get().amount;
            *exchange_with_vol
                .entry(trade.get().exchange)
                .or_insert(Rational::ZERO) += &trade.get().amount;

            trade_volume += &trade.get().amount;
        }

        if trade_volume == Rational::ZERO {
            return None;
        }
        let exchanges = exchange_with_vol.into_iter().collect_vec();

        let maker =
            ExchangePrice { exchanges: exchanges.clone(), price: vxp_maker / &trade_volume };
        let taker =
            ExchangePrice { exchanges: exchanges.clone(), price: vxp_taker / &trade_volume };

        Some((maker, taker))
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, Redefined, PartialEq, Eq)]
#[redefined_attr(derive(
    Debug,
    PartialEq,
    Eq,
    Clone,
    Hash,
    Serialize,
    rSerialize,
    rDeserialize,
    Archive
))]
pub struct CexTrades {
    #[redefined(same_fields)]
    pub exchange: CexExchange,
    pub price:    Rational,
    pub amount:   Rational,
}

/// Its ok that we create 2 of these for pair price and intermediary price
/// as it runs off of borrowed data so there is no overhead we occur
pub struct PairTradeQueue<'a> {
    exchange_depth: FastHashMap<CexExchange, usize>,
    trades:         Vec<(CexExchange, Vec<&'a CexTrades>)>,
}

impl<'a> PairTradeQueue<'a> {
    /// Assumes the trades are sorted based off the side that's passed in
    pub fn new(
        trades: Vec<(CexExchange, Vec<&'a CexTrades>)>,
        execution_quality_pct: Option<FastHashMap<CexExchange, usize>>,
    ) -> Self {
        // calculate the ending index (depth) based of the quality pct for the given
        // exchange and pair.
        // -- quality percent is the assumed percent of good trades the arber is
        // capturing for the relevant pair on a given exchange
        // -- a lower quality percent means we need to assess more trades because it's
        // possible the arber was getting bad execution. Vice versa for a high quality
        // percent
        let exchange_depth = if let Some(quality_pct) = execution_quality_pct {
            trades
                .iter()
                .map(|(exchange, data)| {
                    let length = data.len();
                    let quality = quality_pct.get(exchange).copied().unwrap_or(100);
                    let idx = length - (length * quality / 100);
                    (*exchange, idx)
                })
                .collect::<FastHashMap<_, _>>()
        } else {
            FastHashMap::default()
        };

        Self { exchange_depth, trades }
    }

    fn next_best_trade(&mut self) -> Option<CexTradePtr<'a>> {
        let mut next: Option<CexTradePtr<'a>> = None;

        for (exchange, trades) in &self.trades {
            let exchange_depth = *self.exchange_depth.entry(*exchange).or_insert(0);
            let len = trades.len() - 1;

            // hit max depth
            if exchange_depth > len {
                continue;
            }

            if let Some(trade) = trades.get(len - exchange_depth) {
                if let Some(cur_best) = next.as_ref() {
                    // found a better price

                    if trade.price.gt(&cur_best.get().price) {
                        next = Some(CexTradePtr::new(trade));
                    }
                // not set
                } else {
                    next = Some(CexTradePtr::new(trade));
                }
            }
        }

        // increment ptr
        if let Some(next) = next.as_ref() {
            *self.exchange_depth.get_mut(&next.get().exchange).unwrap() += 1;
        }

        next
    }
}

fn calculate_cross_pair(
    v0: FastHashMap<Address, Vec<MakerTaker>>,
    mut v1: FastHashMap<Address, Vec<MakerTaker>>,
) -> Option<MakerTaker> {
    let (maker, taker): (Vec<_>, Vec<_>) = v0
        .into_iter()
        .flat_map(|(inter, vwam0)| {
            let Some(vwam1) = v1.remove(&inter) else {
                return vec![];
            };

            vwam0
                .into_iter()
                .flat_map(|(maker0, taker0)| {
                    vwam1.iter().map(move |(maker1, taker1)| {
                        let maker_exchanges = maker0
                            .exchanges
                            .iter()
                            .chain(maker1.exchanges.iter())
                            .fold(FastHashMap::default(), |mut a, b| {
                                *a.entry(b.0).or_insert(Rational::ZERO) += &b.1;
                                a
                            })
                            .into_iter()
                            .collect_vec();

                        let taker_exchanges = taker0
                            .exchanges
                            .iter()
                            .chain(taker0.exchanges.iter())
                            .fold(FastHashMap::default(), |mut a, b| {
                                *a.entry(b.0).or_insert(Rational::ZERO) += &b.1;
                                a
                            })
                            .into_iter()
                            .collect_vec();

                        let maker = ExchangePrice {
                            exchanges: maker_exchanges,
                            price:     &maker0.price * &maker1.price,
                        };

                        let taker = ExchangePrice {
                            exchanges: taker_exchanges,
                            price:     &taker0.price * &taker1.price,
                        };
                        (maker, taker)
                    })
                })
                .collect_vec()
        })
        .unzip();

    Some((
        maker.into_iter().max_by_key(|a| a.price.clone())?,
        taker.into_iter().max_by_key(|a| a.price.clone())?,
    ))
}

fn closest<'a>(
    iter: impl Iterator<Item = Vec<&'a CexTradePtr<'a>>>,
    vol: &Rational,
) -> Option<Vec<&'a CexTradePtr<'a>>> {
    // sort from lowest to highest volume returning the first
    // does not return a vec that does not have enough volume to fill the arb
    iter.sorted_by(|a, b| {
        a.iter()
            .map(|t| &t.get().amount)
            .sum::<Rational>()
            .cmp(&b.iter().map(|t| &t.get().amount).sum::<Rational>())
    })
    .find(|set| {
        set.iter()
            .map(|t| &t.get().amount)
            .sum::<Rational>()
            .ge(vol)
    })
}

struct CexTradePtr<'ptr> {
    raw: *const CexTrades,
    /// used to bound the raw ptr so we can't use it if it goes out of scope.
    _p:  PhantomData<&'ptr u8>,
}

impl<'ptr> CexTradePtr<'ptr> {
    fn new(raw: &CexTrades) -> Self {
        Self { raw: raw as *const _, _p: PhantomData }
    }

    fn get(&'ptr self) -> &'ptr CexTrades {
        unsafe { &*self.raw }
    }
}
