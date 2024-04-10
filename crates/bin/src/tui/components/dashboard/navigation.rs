#[derive(Default, Debug)]
pub struct Navigation {
    pub tab_index:          usize,
    pub popup_scroll_state: ScrollbarState,
    show_popup:             bool,
}
