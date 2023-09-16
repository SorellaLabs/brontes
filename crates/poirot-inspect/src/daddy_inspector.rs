use std::{sync::Arc, task::Poll};

use poirot_labeller::Metadata;
use poirot_types::{
    classified_mev::{ClassifiedMev, MevBlock, SpecificMev},
    normalized_actions::Actions,
    tree::TimeTree
};

type InspectorFut<'a> =
    JoinAll<Pin<Box<dyn Future<Output = Vec<(ClassifiedMev, Box<dyn SpecificMev>)>> + Send + 'a>>>;

pub struct DaddyInspector<'a, const N: usize> {
    baby_inspectors:      &'a [&'a Box<dyn Inspector<Mev = Box<dyn SpecificMev>>>; N],
    inspectors_execution: Option<InspectorFut<'a>>,
    core:                 DaddyInspectorCore
}

impl<'a, const N: usize> DaddyInspector<'a, N> {
    pub fn new(baby_inspectors: &'a [&'a Box<dyn Inspector<Mev = dyn SpecificMev>>; N]) -> Self {
        Self { baby_inspectors, inspectors_execution: None, core: DaddyInspectorCore::new() }
    }

    pub fn on_new_tree(&mut self, tree: Arc<TimeTree<Actions>>, meta_data: Arc<Metadata>) {
        self.inspectors_execution = Some(join_all(
            self.baby_inspectors
                .iter()
                .map(|inspector| inspector.process_tree(tree.clone(), metadata.clone()))
        ) as InspectorFut<'a>);
    }

    pub fn is_processing(&self) -> bool {
        self.inspectors_execution.is_some()
    }
}

impl<const N: usize> Stream for DaddyInspector<'_, N> {
    type Item = (MevBlock, Vec<(ClassifiedMev, Box<dyn SpecificMev>)>);

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        todo!()
    }
}

pub struct DaddyInspectorCore {}

impl DaddyInspectorCore {
    pub fn new() -> Self {
        Self {}
    }
}
