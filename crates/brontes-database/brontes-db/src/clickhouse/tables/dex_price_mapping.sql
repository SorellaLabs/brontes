CREATE TABLE brontes.dex_price_mapping 
(
    `block_number` UInt64,
    `tx_idx` UInt64,
    `quote` Array(Tuple(
        Tuple(String, String),
        Tuple(
          Tuple(Array(UInt64), Array(UInt64)),
          Tuple(Array(UInt64), Array(UInt64)),
          Tuple(Array(UInt64), Array(UInt64)),
          Tuple(String, String),
          bool, UInt64
        )
    )),
    `last_updated` UInt64 DEFAULT now()
)
ENGINE = MergeTree()
PRIMARY KEY (`block_number`, `tx_idx`)
ORDER BY(`block_number`, `tx_idx`)
