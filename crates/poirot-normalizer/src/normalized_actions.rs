pub enum Actions {}
pub trait NormalizedAction {
    fn get_action(&self) -> Actions;
}
