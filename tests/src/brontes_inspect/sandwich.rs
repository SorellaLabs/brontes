use std::{fs, str::FromStr, sync::Arc};

use brontes_classifier::{test_utils::build_raw_test_tree, Classifier};
use brontes_core::test_utils::init_trace_parser;
use brontes_database::database::Database;
use brontes_inspect::{sandwich::SandwichInspector, Inspector};
use brontes_types::{normalized_actions::Actions, test_utils::write_tree_as_json, tree::TimeTree};
use reth_primitives::H256;
use tokio::sync::mpsc::unbounded_channel;

#[tokio::test]
async fn process_tree() {
    dotenv::dotenv().ok();
    let block_num = 17891800;

    let (tx, _rx) = unbounded_channel();

    let tracer = init_trace_parser(tokio::runtime::Handle::current().clone(), tx);
    let db = Database::default();
    let classifier = Classifier::new();

    let block = tracer.execute_block(block_num).await.unwrap();
    let metadata = db.get_metadata(block_num).await;
    let tree = Arc::new(classifier.build_tree(block.0, block.1, &metadata));

    //write_tree_as_json(&tree, "./tree.json").await;

    let inspector = SandwichInspector::default();

    let mev = inspector.process_tree(tree.clone(), metadata.into()).await;

    println!("{:?}", mev);

    let actions = tree
        .inspect(
            H256::from_str("0xd8d45bdcb25ba4cb2ecb357a5505d03fa2e67fe6e6cc032ca6c05de75d14f5b5")
                .unwrap(),
            |node| true,
        )
        .into_iter()
        .collect::<Vec<_>>();

    println!("ACTIONSSS: {:?}", actions);
}

#[test]
fn tree_ting() {
    let tree: TimeTree<Actions> =
        serde_json::from_str(&fs::read_to_string("src/brontes_inspect/tree.json").unwrap())
            .unwrap();

    let root = tree
        .roots
        .iter()
        .filter(|r| {
            r.tx_hash
                == H256::from_str(
                    "0xd8d45bdcb25ba4cb2ecb357a5505d03fa2e67fe6e6cc032ca6c05de75d14f5b5",
                )
                .unwrap()
        })
        .collect::<Vec<_>>()[0];

    println!("{:?}", root.head.subactions);
}
