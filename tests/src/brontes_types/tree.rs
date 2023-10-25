use brontes_classifier::test_utils::build_raw_test_tree;
use brontes_core::test_utils::init_trace_parser;
use brontes_database::database::Database;
use brontes_types::test_utils::print_tree_as_json;
use serial_test::serial;
use tokio::sync::mpsc::unbounded_channel;

#[tokio::test]
#[serial]
async fn test_sum() {
    dotenv::dotenv().ok();

    let (tx, _rx) = unbounded_channel();

    let tracer = init_trace_parser(tokio::runtime::Handle::current().clone(), tx);
    let db = Database::default();

    let tree = build_raw_test_tree(tracer, db).await;
    print_tree_as_json(&tree);
}
