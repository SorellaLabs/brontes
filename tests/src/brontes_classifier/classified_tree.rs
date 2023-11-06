use std::{collections::HashMap, fs, str::FromStr};

use brontes_classifier::{test_utils::*, Classifier};
use brontes_core::{
    decoding::vm_linker::link_vm_to_trace,
    test_utils::{init_trace_parser, TestTraceResults, TestTransactionReceipt},
};
use brontes_database::database::Database;
use brontes_types::{
    test_utils::{print_tree_as_json, write_tree_as_json},
    tree::TimeTree,
};
use reth_primitives::{H160, H256};
use reth_rpc_types::{
    trace::parity::{TransactionTrace, VmTrace},
    Log,
};
use tokio::sync::mpsc::unbounded_channel;

use crate::UNIT_TESTS_BLOCK_NUMBER;

/// Uniswap V2 - Bone Shibaswap <> Weth
fn token_mapping() -> HashMap<H160, (H160, H160)> {
    let mut map = HashMap::new();
    map.insert(
        H160::from_str("0xF7d31825946e7fD99eF07212d34B9Dad84C396b7").unwrap(),
        (
            H160::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap(),
            H160::from_str("0x9813037ee2218799597d83d4a5b6f3b6778218d9").unwrap(),
        ),
    );
    map
}

async fn test_classified_tree() {
    let (tx, _rx) = unbounded_channel();
    let tracer = init_trace_parser(tokio::runtime::Handle::current().clone(), tx);

    let db = Database::default();
    let classifier = Classifier::new();

    let (traces, header, metadata) =
        get_traces_with_meta(&tracer, &db, UNIT_TESTS_BLOCK_NUMBER).await;

    let tree = classifier.build_tree(traces, header, &metadata);
}

#[tokio::test]
async fn test_try_classify_unknown_exchanges() {
    let (tx, _rx) = unbounded_channel();
    let tracer = init_trace_parser(tokio::runtime::Handle::current().clone(), tx);

    let db = Database::default();
    let classifier = Classifier::new();

    let token_mapping = token_mapping();

    let tree = build_raw_test_tree(&tracer, &db, UNIT_TESTS_BLOCK_NUMBER).await;
    print_tree_as_json(&tree);

    let root = tree
        .roots
        .into_iter()
        .filter(|r| {
            r.tx_hash
                == H256::from_str(
                    "0x89828843c77b22dc3da366241e5ed4a4ab6310288ad6572c1fb607d9abbf2654",
                )
                .unwrap()
        })
        .collect::<Vec<_>>();

    let mut test_tree = TimeTree {
        roots: root,
        header: tree.header,
        eth_prices: tree.eth_prices,
        avg_priority_fee: tree.avg_priority_fee,
    };

    print_tree_as_json(&test_tree);
    println!("\n\n\n\n");

    helper_try_classify_unknown_exchanges2(&classifier, &mut test_tree);

    let actions = test_tree.inspect_all(|node| !node.data.is_unclassified());
    println!("{:?}", actions);

    //print_tree_as_json(&test_tree);
    //println!("\n\n\n\n");
}

#[tokio::test]
async fn test_classify_node() {
    dotenv::dotenv().ok();

    // testing 0xd8d45bdcb25ba4cb2ecb357a5505d03fa2e67fe6e6cc032ca6c05de75d14f5b5
    let block_num = 17891800;

    let (tx, _rx) = unbounded_channel();

    let tracer = init_trace_parser(tokio::runtime::Handle::current().clone(), tx);

    let block = tracer.execute_block(block_num).await.unwrap(); // searching for
    let tx_trace = block
        .0
        .into_iter()
        .filter(|tx| {
            tx.tx_hash
                == H256::from_str(
                    "0xd8d45bdcb25ba4cb2ecb357a5505d03fa2e67fe6e6cc032ca6c05de75d14f5b5",
                )
                .unwrap()
        })
        .collect::<Vec<_>>();

    let classifier = Classifier::new();

    let db = Database::default();

    let tree = build_raw_test_tree(&tracer, &db, block_num)
        .await
        .roots
        .into_iter()
        .filter(|r| {
            r.tx_hash
                == H256::from_str(
                    "0xd8d45bdcb25ba4cb2ecb357a5505d03fa2e67fe6e6cc032ca6c05de75d14f5b5",
                )
                .unwrap()
        })
        .collect::<Vec<_>>();

    //println!("{:?}", tree.len());

    let metadata = db.get_metadata(block_num).await;
    let raw_tree = TimeTree {
        roots: tree,
        header: block.1.clone(),
        avg_priority_fee: 0,
        eth_prices: metadata.eth_prices.clone(),
    };

    let classified_tree = classifier.build_tree(tx_trace, block.1, &metadata);

    print_tree_as_json(&raw_tree);

    println!("\n\n\n\n\n\n\n\n\n\n\n\n\n\n");
    print_tree_as_json(&classified_tree);

    //helper_classify_node(&classifier, tx_trace.trace, 0);
}

#[tokio::test]
async fn ugh() {
    dotenv::dotenv().ok();

    // testing 0xd42987b923b9e10de70df67b2bb57eefe21dec0a4c0372d3bcbdb69feb34dff4
    let block_num = 18429722;

    let (tx, _rx) = unbounded_channel();

    let tracer = init_trace_parser(tokio::runtime::Handle::current().clone(), tx);

    let block = tracer.execute_block(block_num).await.unwrap(); // searching for
    let tx_trace = block
        .0
        .into_iter()
        .filter(|tx| {
            tx.tx_hash
                == H256::from_str(
                    "0xd42987b923b9e10de70df67b2bb57eefe21dec0a4c0372d3bcbdb69feb34dff4",
                )
                .unwrap()
        })
        .collect::<Vec<_>>();

    let classifier = Classifier::new();

    let db = Database::default();

    let tree = build_raw_test_tree(&tracer, &db, block_num)
        .await
        .roots
        .into_iter()
        .filter(|r| {
            r.tx_hash
                == H256::from_str(
                    "0xd42987b923b9e10de70df67b2bb57eefe21dec0a4c0372d3bcbdb69feb34dff4",
                )
                .unwrap()
        })
        .collect::<Vec<_>>();

    //println!("{:?}", tree.len());

    let metadata = db.get_metadata(block_num).await;
    let raw_tree = TimeTree {
        roots: tree,
        header: block.1.clone(),
        avg_priority_fee: 0,
        eth_prices: metadata.eth_prices.clone(),
    };

    let classified_tree = classifier.build_tree(tx_trace, block.1, &metadata);

    write_tree_as_json(&classified_tree, "./tree.json").await;

    print_tree_as_json(&raw_tree);

    println!("\n\n\n\n\n\n\n\n\n\n\n\n\n\n");
    print_tree_as_json(&classified_tree);

    //helper_classify_node(&classifier, tx_trace.trace, 0);
}
