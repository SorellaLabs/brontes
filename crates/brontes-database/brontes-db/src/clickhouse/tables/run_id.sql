CREATE TABLE brontes.run_id 
(
    `run_id` UInt64,
    `last_updated` UInt64 DEFAULT now()
)
ENGINE = MergeTree()
ORDER BY run_id
SETTINGS index_granularity = 8192