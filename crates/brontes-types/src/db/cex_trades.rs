use std::{collections::HashMap, marker::PhantomData};

use alloy_primitives::Address;
use itertools::Itertools;
use malachite::{
    num::basic::traits::{One, Zero},
    Rational,
};
use rayon::iter::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};

use super::cex::CexExchange;
use crate::pair::Pair;

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

/// All cex trades for a given period, grouped into there dedicated markout
/// periods
#[derive(Debug, Clone)]
pub struct CexTradeMap(pub Vec<HashMap<CexExchange, HashMap<Pair, Vec<CexTrades>>>>);

impl CexTradeMap {
    /// goes through each set of trade periods calculating
    /// the best execution cost across all exchanges while adhering to
    /// the execution quality params that are passed in.
    /// NOTE: the prices returned are fee adjusted.
    pub fn get_price(
        &self,
        exchanges: &[CexExchange],
        pair: &Pair,
        volume: &Rational,
        baskets: usize,
        quality: Option<HashMap<CexExchange, HashMap<Pair, usize>>>,
    ) -> Option<MakerTaker> {
        let (maker, taker): (Vec<_>, Vec<_>) = self
            .0
            .par_iter()
            .filter_map(|bin| {
                CexTradeBasket(bin).get_vwam_price(
                    exchanges,
                    pair,
                    volume,
                    baskets,
                    quality.as_ref(),
                )
            })
            .unzip();

        Some((
            maker.into_iter().max_by_key(|a| a.price.clone())?,
            taker.into_iter().max_by_key(|a| a.price.clone())?,
        ))
    }
}

// cex trades are sorted from lowest fill price to highest fill price
struct CexTradeBasket<'a>(pub &'a HashMap<CexExchange, HashMap<Pair, Vec<CexTrades>>>);

type FoldVWAM = HashMap<Address, Vec<MakerTaker>>;

impl CexTradeBasket<'_> {
    fn get_vwam_price(
        &self,
        exchanges: &[CexExchange],
        pair: &Pair,
        volume: &Rational,
        baskets: usize,
        quality: Option<&HashMap<CexExchange, HashMap<Pair, usize>>>,
    ) -> Option<MakerTaker> {
        self.get_vwam_no_intermediary(exchanges, pair, volume, baskets, quality)
            .or_else(|| self.get_vwam_via_intermediary(exchanges, pair, volume, baskets, quality))
    }

    fn get_vwam_via_intermediary(
        &self,
        exchanges: &[CexExchange],
        pair: &Pair,
        volume: &Rational,
        baskets: usize,
        quality: Option<&HashMap<CexExchange, HashMap<Pair, usize>>>,
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

        let (pair0_vwams, pair1_vwams) = self
            .0
            .keys()
            .filter(|ex| exchanges.contains(ex))
            .map(|exchange| {
                exchange
                    .most_common_quote_assets()
                    .into_par_iter()
                    .filter_map(|intermediary| {
                        let pair0 = Pair(pair.0, intermediary);
                        let pair1 = Pair(intermediary, pair.1);
                        Some((
                            (
                                intermediary,
                                self.get_vwam_no_intermediary(
                                    exchanges, &pair0, volume, baskets, quality,
                                )?,
                            ),
                            (
                                intermediary,
                                self.get_vwam_no_intermediary(
                                    exchanges, &pair1, volume, baskets, quality,
                                )?,
                            ),
                        ))
                    })
                    .fold(
                        || (HashMap::new(), HashMap::new()),
                        |(mut pair0_vwam, mut pair1_vwam), ((iter0, prices0), (iter1, prices1))| {
                            pair0_vwam.entry(iter0).or_insert(vec![]).push(prices0);
                            pair1_vwam.entry(iter1).or_insert(vec![]).push(prices1);
                            (pair0_vwam, pair1_vwam)
                        },
                    )
                    .reduce(|| (HashMap::new(), HashMap::new()), fold_fn)
            })
            .fold((HashMap::new(), HashMap::new()), fold_fn);

        calculate_cross_pair(pair0_vwams, pair1_vwams)
    }

    fn get_vwam_no_intermediary(
        &self,
        exchanges: &[CexExchange],
        pair: &Pair,
        volume: &Rational,
        baskets: usize,
        quality: Option<&HashMap<CexExchange, HashMap<Pair, usize>>>,
    ) -> Option<MakerTaker> {
        let quality_pct = quality.map(|map| {
            map.into_iter()
                .map(|(k, v)| (*k, v.get(pair).copied().unwrap_or(BASE_EXECUTION_QUALITY)))
                .collect::<HashMap<_, _>>()
        });

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
                            .into_iter()
                            .filter(|f| f.amount.le(&max_vol_per_trade))
                            .collect_vec()
                    })?,
                ))
            })
            .collect::<Vec<_>>();

        let trade_queue = PairTradeQueue::new(trades, quality_pct);
        self.get_most_accurate_basket(trade_queue, volume, baskets)
    }

    fn get_most_accurate_basket<'a>(
        &self,
        mut queue: PairTradeQueue<'a>,
        volume: &Rational,
        baskets: usize,
    ) -> Option<MakerTaker> {
        let mut trades = Vec::new();

        let volume_amount = volume * Rational::from(baskets);
        let mut cur_vol = Rational::ZERO;

        while volume_amount.gt(&cur_vol) {
            let Some(next) = queue.next_best_trade() else { break };
            // we do this due to the sheer amount of trades we have and to not have to copy.
            // all of this is safe
            cur_vol += &next.get().amount;

            trades.push(next);
        }

        let closest = closest(
            trades.iter().map(|t| vec![t]).chain(
                trades
                    .iter()
                    .combinations(2)
                    .chain(trades.iter().combinations(3))
                    .chain(trades.iter().combinations(4)),
            ),
            volume,
        )?;

        let mut vxp_maker = Rational::ZERO;
        let mut vxp_taker = Rational::ZERO;
        let mut trade_volume = Rational::ZERO;
        let mut exchange_with_vol = HashMap::new();

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
            return None
        }
        let exchanges = exchange_with_vol.into_iter().collect_vec();

        let maker =
            ExchangePrice { exchanges: exchanges.clone(), price: vxp_maker / &trade_volume };
        let taker =
            ExchangePrice { exchanges: exchanges.clone(), price: vxp_taker / &trade_volume };

        Some((maker, taker))
    }
}

#[derive(Debug, Clone)]
pub struct CexTrades {
    pub exchange: CexExchange,
    pub price:    Rational,
    pub amount:   Rational,
}

/// Its ok that we create 2 of these for pair price and intermediary price
/// as it runs off of borrowed data so there is no overhead we occur
pub struct PairTradeQueue<'a> {
    exchange_depth: HashMap<CexExchange, usize>,
    trades:         Vec<(CexExchange, Vec<&'a CexTrades>)>,
}

impl<'a> PairTradeQueue<'a> {
    /// Assumes the trades are sorted based off the side that's passed in
    pub fn new(
        trades: Vec<(CexExchange, Vec<&'a CexTrades>)>,
        execution_quality_pct: Option<HashMap<CexExchange, usize>>,
    ) -> Self {
        // calculate the starting index based of the quality pct for the given exchange
        // and pair.
        let exchange_depth = if let Some(quality_pct) = execution_quality_pct {
            trades
                .iter()
                .map(|(exchange, data)| {
                    let length = data.len();
                    let quality = quality_pct.get(exchange).copied().unwrap_or(100);
                    let idx = length - (length * quality / 100);
                    (*exchange, idx)
                })
                .collect::<HashMap<_, _>>()
        } else {
            HashMap::default()
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
                continue
            }

            if let Some(trade) = trades.get(len - exchange_depth) {
                if let Some(cur_best) = next.as_ref() {
                    // found a better price

                    if trade.price.gt(&cur_best.get().price) {
                        next = Some(CexTradePtr::new(*trade));
                    }
                // not set
                } else {
                    next = Some(CexTradePtr::new(*trade));
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
    v0: HashMap<Address, Vec<MakerTaker>>,
    mut v1: HashMap<Address, Vec<MakerTaker>>,
) -> Option<MakerTaker> {
    let (maker, taker): (Vec<_>, Vec<_>) = v0
        .into_iter()
        .flat_map(|(inter, vwam0)| {
            let Some(vwam1) = v1.remove(&inter) else { return vec![] };

            vwam0
                .into_iter()
                .flat_map(|(maker0, taker0)| {
                    vwam1.iter().map(move |(maker1, taker1)| {
                        let maker_exchanges = maker0
                            .exchanges
                            .iter()
                            .chain(maker1.exchanges.iter())
                            .fold(HashMap::new(), |mut a, b| {
                                *a.entry(b.0).or_insert(Rational::ZERO) += &b.1;
                                a
                            })
                            .into_iter()
                            .collect_vec();

                        let taker_exchanges = taker0
                            .exchanges
                            .iter()
                            .chain(taker0.exchanges.iter())
                            .fold(HashMap::new(), |mut a, b| {
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
        Self { raw: raw as *const _, _p: PhantomData::default() }
    }

    fn get(&'ptr self) -> &'ptr CexTrades {
        unsafe { &*self.raw }
    }
}
