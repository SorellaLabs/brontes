use std::{fs, str::FromStr, sync::Arc};

use brontes_classifier::{test_utils::build_raw_test_tree, Classifier};
use brontes_core::test_utils::init_trace_parser;
use brontes_database::database::Database;
use brontes_inspect::{sandwich::SandwichInspector, Inspector};
use brontes_types::{normalized_actions::Actions, test_utils::write_tree_as_json, tree::TimeTree};
use reth_primitives::H256;
use serial_test::serial;
use tokio::sync::mpsc::unbounded_channel;

#[tokio::test]
#[serial]
async fn process_tree() {
    dotenv::dotenv().ok();
    let block_num = 17891800;

    let (tx, _rx) = unbounded_channel();

    let tracer = init_trace_parser(tokio::runtime::Handle::current().clone(), tx);
    let db = Database::default();
    let classifier = Classifier::new();

    let block = tracer.execute_block(block_num).await.unwrap();
    let metadata = db.get_metadata(block_num).await;

    let tx = block.0.clone().into_iter().take(3).collect::<Vec<_>>();
    let tree = Arc::new(classifier.build_tree(tx, block.1, &metadata));

    //write_tree_as_json(&tree, "./tree.json").await;

    let inspector = SandwichInspector::default();

    let mev = inspector.process_tree(tree.clone(), metadata.into()).await;

    assert!(
        mev[0].0.tx_hash
            == H256::from("0x80b53e5e9daa6030d024d70a5be237b4b3d5e05d30fdc7330b62c53a5d3537de")
    );

    println!("{:?}", mev);
}
