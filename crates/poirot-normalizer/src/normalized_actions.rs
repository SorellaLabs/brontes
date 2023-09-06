pub enum Actions {}

pub trait NormalizedAction: Clone {
    fn get_action(&self) -> Actions;
}
