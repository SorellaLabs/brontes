use brontes_classifier::test_utils::build_raw_test_tree;
use brontes_core::test_utils::init_trace_parser;
use brontes_database::database::Database;
use brontes_inspect::sandwich::SandwichInspector;
use tokio::sync::mpsc::unbounded_channel;

async fn process_tree() {
    dotenv::dotenv().ok();
    let block_num = 17891800;

    let (tx, _rx) = unbounded_channel();

    let tracer = init_trace_parser(tokio::runtime::Handle::current().clone(), tx);
    let db = Database::default();

    let mut tree = build_raw_test_tree(&tracer, &db, block_num).await;

    let inspector = SandwichInspector::default();
}
