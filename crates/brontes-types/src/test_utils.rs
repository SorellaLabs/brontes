use crate::{normalized_actions::Actions, tree::TimeTree};

pub fn print_tree_as_json(tree: &TimeTree<Actions>) {
    let serialized_tree = serde_json::to_string_pretty(tree).unwrap();
    println!("{}", serialized_tree);
}

pub async fn write_tree_as_json(tree: &TimeTree<Actions>, path: &str) {
    let serialized_tree = serde_json::to_string_pretty(tree).unwrap();
    tokio::fs::write(path, serialized_tree).await.unwrap();
}
