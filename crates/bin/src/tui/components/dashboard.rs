use std::{
    sync::{Arc, Mutex},
    thread, time,
};

use ansi_to_tui::IntoText;
use brontes_types::mev::{
    bundle::Bundle,
    events::{Action, TuiEvents},
    Mev, MevBlock,
};
use crossterm::event::{KeyCode, KeyEvent};
use eyre::Result; //
use itertools::Itertools;
use log::*;
use ratatui::{prelude::*, widgets::*};
use tokio::sync::mpsc::UnboundedSender;
use tracing::info;
use tui_logger::*;
use polars::prelude::*;

use super::{Component, Frame};
use crate::{
    get_symbols_from_transaction_accounting,
    tui::{config::Config, theme::THEME, tui::Event},
};

#[derive(Default, Debug)]
pub struct Dashboard {
    command_tx: Option<UnboundedSender<Action>>,
    config: Config,
    mevblocks: Arc<Mutex<Vec<MevBlock>>>,
    mev_bundles: Arc<Mutex<Vec<Bundle>>>, // Shared state for MevBlocks
    data: Vec<(&'static str, u64)>,
    stream_table_state: TableState,
    show_popup: bool,
    pub popup_scroll_position: u16,
    pub popup_scroll_state: ScrollbarState,
    pub progress_counter: Option<u16>,

    leaderboard: Vec<(&'static str, u64)>,
}

impl Dashboard {
    pub fn new(mevblocks: Arc<Mutex<Vec<MevBlock>>>, mev_bundles: Arc<Mutex<Vec<Bundle>>>) -> Self {
        Self {
            //log_scroll: 0,
            mevblocks,
            mev_bundles,
            show_popup: false,
            data: vec![
                ("Sandwich", 0),
                ("Jit Sandwich", 0),
                ("Cex-Dex", 0),
                ("Jit", 0),
                ("Atomic Backrun", 0),
                ("Liquidation", 0),
            ],

            stream_table_state: TableState::default().with_selected(Some(0)),

            leaderboard: vec![
                ("jaredfromsubway.eth", 1_200_000),
                ("0x23892382394..212", 1_100_000),
                ("0x13897682394..243", 1_000_000),
                ("0x33899882394..223", 900_000),
                ("0x43894082394..265", 800_000),
                ("0x53894082394..283", 700_000),
                ("0x83894082394..293", 600_000),
                // Repeat as necessary
            ],
            ..Default::default()
        }
    }

    pub fn next(&mut self) {
        if self.show_popup {
            self.popup_scroll_position = self.popup_scroll_position.saturating_sub(1);
            self.popup_scroll_state = self
                .popup_scroll_state
                .position(self.popup_scroll_position as usize);
        } else {
            let i = match self.stream_table_state.selected() {
                Some(i) => {
                    let mevblocks_guard: std::sync::MutexGuard<'_, Vec<Bundle>> =
                        self.mev_bundles.lock().unwrap();

                    if mevblocks_guard.len() > 0 {
                        if i == 0 {
                            mevblocks_guard.len() - 1
                        } else {
                            i - 1
                        }
                    } else {
                        0
                    }
                }
                None => 0,
            };
            self.stream_table_state.select(Some(i));
        }
    }

    pub fn previous(&mut self) {
        if self.show_popup {
            self.popup_scroll_position = self.popup_scroll_position.saturating_add(1);
            self.popup_scroll_state = self
                .popup_scroll_state
                .position(self.popup_scroll_position as usize);
        } else {
            let i = match self.stream_table_state.selected() {
                Some(i) => {
                    let mevblocks_guard: std::sync::MutexGuard<'_, Vec<Bundle>> =
                        self.mev_bundles.lock().unwrap();

                    if mevblocks_guard.len() > 0 {
                        if i >= mevblocks_guard.len() - 1 {
                            0
                        } else {
                            i + 1
                        }
                    } else {
                        0
                    }
                }
                None => 0,
            };

            self.stream_table_state.select(Some(i));
        }
    }


// Function to convert a Vec<Bundle> to a Polars DataFrame
fn bundles_to_dataframe(bundles:  Vec<Bundle>) -> Result<DataFrame> {
    //info!("bundles_to_dataframe_entered");

    let mut block_numbers = Vec::new();
    let mut tx_indexes = Vec::new();
    let mut mev_types = Vec::new();
    let mut symbols = Vec::new();
    let mut protocols = Vec::new();
    let mut eoas = Vec::new();
    let mut mev_contracts = Vec::new();
    let mut profits_usd = Vec::new();
    let mut bribes_usd = Vec::new();

    for bundle in bundles.iter() {
        block_numbers.push(bundle.header.block_number);
        tx_indexes.push(bundle.header.tx_index);
        mev_types.push(bundle.header.mev_type.to_string());
        symbols.push(get_symbols_from_transaction_accounting!(&bundle.header.balance_deltas)); // Assuming this macro/functionality
        protocols.push(bundle.data.protocols().iter().map(|p| p.to_string()).sorted().join(", "));
        eoas.push(bundle.header.eoa.to_string());
        mev_contracts.push(
            bundle
                .header
                .mev_contract
                .as_ref()
                .map(|address| address.to_string())
                .unwrap_or_else(|| "Not an Mev Contract".to_string()),
        );
        profits_usd.push(bundle.header.profit_usd);
        bribes_usd.push(bundle.header.bribe_usd);
    }

    let df = DataFrame::new(vec![
        Series::new("Block Number", &block_numbers),
        Series::new("Tx Index", &tx_indexes),
        Series::new("MEV Type", &mev_types),
        Series::new("Symbols", &symbols),
        Series::new("Protocols", &protocols),
        Series::new("EOA", &eoas),
        Series::new("MEV Contract", &mev_contracts),
        Series::new("Profit USD", &profits_usd),
        Series::new("Bribe USD", &bribes_usd),
    ])?;
    //info!("bundles_to_dataframe_finish");

    Ok(df)
}



fn dataframe_to_table_rows(df: &DataFrame) -> Vec<Row> {
    //info!("dataframe_table_rows_entered");

    let height = 1;
    let bottom_margin = 0;

    // This approach assumes you know the schema of your DataFrame
    // and can access each column as needed. It's not a direct replacement
    // for a row-wise iterator but demonstrates manual assembly of rows.
    let num_rows = df.height();
    let mut rows = Vec::with_capacity(num_rows);

    for i in 0..num_rows {
        let mut cells = Vec::new();
        for series in df.get_columns() {
            // Assuming you can handle the specific type of each column,
            // you would extract the value for the current row (`i`) from each column's ChunkedArray.
            // This snippet assumes a generic approach, not specific to any data type.
            let value_str = series.get(i).unwrap().to_string();
            cells.push(Cell::from(value_str));
        }
        rows.push(Row::new(cells).height(height).bottom_margin(bottom_margin));
    }
    //info!("dataframe_table_rows_finish");

    rows
}

    fn draw_livestream(widget: &mut Dashboard, area: Rect, buf: &mut Buffer) {
        let selected_style = Style::default().add_modifier(Modifier::REVERSED);
        let normal_style = Style::default().bg(Color::Blue);

        let header_cells = [
            "Block#",
            "Tx Index",
            "MEV Type",
            "Tokens",
            "Protocols",
            "From",
            "Mev Contract",
            "Profit",
            "Cost",
        ]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::White)));
        let header = Row::new(header_cells)
            .style(normal_style)
            .height(1)
            .bottom_margin(1);

        let mevblocks_guard: std::sync::MutexGuard<'_, Vec<Bundle>> = widget.mev_bundles.lock().unwrap();


let df = Self::bundles_to_dataframe(mevblocks_guard.clone()).unwrap();
let rows = Self::dataframe_to_table_rows(&df);
//println!("rows_created");


/*
        let rows = mevblocks_guard.iter().map(|item| {
            let protocols = item.data.protocols();
            let mut protocol_names = protocols.iter().map(|p| p.to_string()).collect::<Vec<_>>();
            protocol_names.sort();
            let protocol_list = protocol_names.join(", ");

            let height = 1;
            let cells = vec![
                item.header.block_number.to_string(),
                item.header.tx_index.to_string(),
                item.header.mev_type.to_string(),
                get_symbols_from_transaction_accounting!(&item.header.balance_deltas),
                protocol_list,
                item.header.eoa.to_string(),
                item.header
                    .mev_contract
                    .as_ref()
                    .map(|address| address.to_string())
                    .unwrap_or("Not an Mev Contract".to_string()),
                item.header.profit_usd.to_string(),
                item.header.bribe_usd.to_string(),
            ]
            .iter()
            .map(|s| Cell::from(s.to_string())) // Convert each String to a Cell
            .collect::<Vec<Cell>>(); // Collect into a Vec<Cell>

            Row::new(cells).height(height as u16).bottom_margin(0)
        });
*/
        let t = Table::new(
            rows,
            [
                Constraint::Max(10),
                Constraint::Min(5),
                Constraint::Min(20),
                Constraint::Min(20),
                Constraint::Min(20),
                Constraint::Min(32),
                Constraint::Min(32),
                Constraint::Max(10),
                Constraint::Max(10),
            ],
        )
        .header(header)
        .block(Block::default().borders(Borders::ALL).title("Live Stream"))
        .highlight_style(selected_style)
        .highlight_symbol(">> ");

        ratatui::widgets::StatefulWidget::render(t, area, buf, &mut widget.stream_table_state);
    }

    fn draw_leaderboard(widget: &Dashboard, area: Rect, buf: &mut Buffer) {
        let barchart = BarChart::default()
        .block(Block::default().borders(Borders::ALL).title("Leaderboard"))
        //.data(&widget.leaderboard.iter().map(|x| (x[0], x[1].parse().unwrap())).collect::<Vec<_>>())
        .data(&widget.leaderboard)
        .bar_width(1)
        .bar_gap(0)
        .bar_set(symbols::bar::NINE_LEVELS)
        .value_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Green)
                .add_modifier(Modifier::ITALIC),
        )
        .direction(Direction::Horizontal)
        .label_style(Style::default().fg(Color::Yellow))
        .bar_style(Style::default().fg(Color::Green));
        barchart.render(area, buf);
    }





    #[allow(unused_variables)]
    fn draw_charts(widget: &mut Dashboard, area: Rect, buf: &mut Buffer) {
        // Initialize counters
        let mut sandwich_total = 0;
        let mut cex_dex_total = 0;
        let mut jit_total = 0;
        let mut jit_sandwich_total = 0;
        let mut atomic_backrun_total = 0;
        let mut liquidation_total = 0;

        let mevblocks_guard: std::sync::MutexGuard<'_, Vec<MevBlock>> =
            widget.mevblocks.lock().unwrap();

        // Aggregate counts
        for item in mevblocks_guard.iter() {
            sandwich_total += item.mev_count.sandwich_count.unwrap_or(0);
            cex_dex_total += item.mev_count.cex_dex_count.unwrap_or(0);
            jit_total += item.mev_count.jit_count.unwrap_or(0);
            jit_sandwich_total += item.mev_count.jit_sandwich_count.unwrap_or(0);
            atomic_backrun_total += item.mev_count.atomic_backrun_count.unwrap_or(0);
            liquidation_total += item.mev_count.liquidation_count.unwrap_or(0);
        }

        // Construct the final Vec<(&str, u64)> with the total counts
        let data: Vec<(&str, u64)> = vec![
            ("Sandwich", sandwich_total),
            ("Cex-Dex", cex_dex_total),
            ("Jit", jit_total),
            ("Jit Sandwich", jit_sandwich_total),
            ("Atomic Backrun", atomic_backrun_total),
            ("Liquidation", liquidation_total),
        ];

        let barchart = BarChart::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Count of MEV Types"),
            )
            .data(&data)
            .bar_width(1)
            .bar_gap(0)
            .bar_set(symbols::bar::NINE_LEVELS)
            .value_style(
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Green)
                    .add_modifier(Modifier::ITALIC),
            )
            .direction(Direction::Horizontal)
            .label_style(Style::default().fg(Color::Yellow))
            .bar_style(Style::default().fg(Color::Green));
        barchart.render(area, buf);
    }


    fn render_bottom_bar(&self, area: Rect, buf: &mut Buffer) {
        let keys = [
            ("Q/Esc", "Quit"),
            ("Tab", "Next Tab"),
            ("BackTab", "Previous Tab"),
            ("↑/w", "Up"),
            ("↓/s", "Down"),
            ("↵", "Open/Close Mev Details"),
        ];
        let spans = keys
            .iter()
            .flat_map(|(key, desc)| {
                let key = Span::styled(format!(" {} ", key), THEME.key_binding.key);
                let desc = Span::styled(format!(" {} ", desc), THEME.key_binding.description);
                [key, desc]
            })
            .collect_vec();
        Paragraph::new(Line::from(spans))
            .alignment(Alignment::Center)
            .fg(Color::Indexed(236))
            .bg(Color::Indexed(232))
            .render(area, buf);
    }

    /// helper function to create a centered rect using up certain percentage of
    /// the available rect `r`
    fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
        let popup_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ])
            .split(r);

        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ])
            .split(popup_layout[1])[1]
    }

    fn draw_logs(_widget: &Dashboard, area: Rect, buf: &mut Buffer) {
        TuiLoggerSmartWidget::default()
            .style_error(Style::default().fg(Color::Red))
            .style_debug(Style::default().fg(Color::Green))
            .style_warn(Style::default().fg(Color::Yellow))
            .style_trace(Style::default().fg(Color::Magenta))
            .style_info(Style::default().fg(Color::Cyan))
            .output_separator(':')
            .output_timestamp(Some("%H:%M:%S".to_string()))
            .output_level(Some(TuiLoggerLevelOutput::Abbreviated))
            .output_target(true)
            .output_file(true)
            .output_line(true)
            .title_target("Target Selector")
            .title_log("Logs")
            .render(area, buf);
    }

    fn update_progress_bar(&mut self, event: Action, value: Option<u16>) {
        trace!(target: "App", "Updating progress bar {:?}",event);
        self.progress_counter = value;
        if value.is_none() {
            info!(target: "App", "Background task finished");
        }
    }

    fn render_progress(&self, area: Rect, buf: &mut Buffer) {
        let progress = self.progress_counter.unwrap_or(0);
        Gauge::default()
            .block(Block::bordered().title("PROGRESS:"))
            .gauge_style((Color::White, Modifier::ITALIC))
            .percent(progress)
            .render(area, buf);
    }


    /// A simulated task that sends a counter value to the UI ranging from 0 to
    /// 100 every second.
    fn progress_task(tx: UnboundedSender<Action>) -> Result<()> {
        for progress in 0..100 {
            debug!(target:"progress-task", "Send progress to UI thread. Value: {:?}", progress);
            tx.send(Action::ProgressChanged(Some(progress)))?;

            trace!(target:"progress-task", "Sleep one second");
            thread::sleep(time::Duration::from_millis(1000));
        }
        info!(target:"progress-task", "Progress task finished");
        tx.send(Action::ProgressChanged(None))?;
        Ok(())
    }
}

impl Component for Dashboard {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        self.command_tx = Some(tx);
        Ok(())
    }

    fn register_config_handler(&mut self, config: Config) -> Result<()> {
        self.config = config;
        Ok(())
    }

    fn name(&self) -> String {
        "Dashboard".to_string()
    }

    fn handle_key_events(&mut self, key: KeyEvent) -> Result<Option<Action>> {
        match key.code {
            KeyCode::Enter => {
                self.popup_scroll_position = 0;
                self.popup_scroll_state = self
                    .popup_scroll_state
                    .position(self.popup_scroll_position as usize);

                self.show_popup = !self.show_popup;
            }
            KeyCode::Up => {
                self.next();
            }
            KeyCode::Down => {
                self.previous();
            }
            _ => (),
        };

        Ok(Some(Action::Tick))
    }

    fn handle_events(&mut self, event: Option<Event>) -> Result<Option<Action>> {
        let r = match event {
            Some(Event::Key(key_event)) => self.handle_key_events(key_event)?,
            Some(Event::Mouse(mouse_event)) => self.handle_mouse_events(mouse_event)?,
            _ => None,
        };
        Ok(r)
    }

    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::Tick => {}
            Action::Tui(tui_event) => {
                match tui_event {
                    TuiEvents::MevBlockMetricReceived(mevblock) => {
                        let mut blocks: std::sync::MutexGuard<'_, Vec<MevBlock>> =
                            self.mevblocks.lock().unwrap();
                        blocks.push(mevblock.clone()); // Store received block
                    }
                    TuiEvents::MevBundleEventReceived(bundle) => {
                        let mut bundles: std::sync::MutexGuard<'_, Vec<Bundle>> =
                            self.mev_bundles.lock().unwrap();
                        bundles.extend(bundle.into_iter());
                    }
                }
            }
            _ => {}
        }
        Ok(None)
    }

    fn init(&mut self, _area: Rect) -> Result<()> {
        Dashboard::new(self.mevblocks.clone(), self.mev_bundles.clone());
        info!("Starting progress task");
        
        // TODO: this can come from anywhere
        // let progress_tx = self.command_tx.clone().unwrap();
        // thread::spawn(move ||
        // Self::progress_task(self.command_tx.unwrap()).unwrap());
        // Self::progress_task(progress_tx).unwrap();

        Ok(())
    }

    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        let area = area.inner(&Margin { vertical: 1, horizontal: 4 });

        let template = Layout::default()
            .constraints([
                Constraint::Length(1),
                Constraint::Min(8),
                Constraint::Length(3),
                Constraint::Length(1),
            ])
            .split(area);

        let chunks = Layout::default()
            .constraints([Constraint::Length(9), Constraint::Min(20), Constraint::Length(8)])
            .split(template[1]);

        let sub_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[0]);

        let buf = f.buffer_mut();

        Self::draw_charts(self, sub_layout[0], buf);
        Self::draw_leaderboard(self, sub_layout[1], buf);
        Self::draw_livestream(self, chunks[1], buf);
        Self::draw_logs(self, chunks[2], buf);
        Self::render_progress(self, template[2], buf);
        Self::render_bottom_bar(self, template[3], buf);
        if self.show_popup {
            if let Some(_selected_index) = self.stream_table_state.selected() {
                let block = Block::default()
                    .title("MEV Details")
                    .borders(Borders::ALL)
                    .padding(Padding::horizontal(4));

                let area = Self::centered_rect(80, 80, area);
                //Self::show_popup(self,area);
                f.render_widget(Clear, area); //this clears out the background
                let paragraph = Paragraph::new("Hello, world!");
                f.render_widget(paragraph, area);

                let mevblocks_guard: std::sync::MutexGuard<'_, Vec<Bundle>> =
                    self.mev_bundles.lock().unwrap();

                let text = mevblocks_guard[self.stream_table_state.selected().unwrap()]
                    .to_string()
                    .into_text();

                let paragraph = Paragraph::new(text.unwrap())
                    .block(block)
                    .scroll((self.popup_scroll_position, 0));

                f.render_widget(paragraph, area);

                f.render_stateful_widget(
                    Scrollbar::default()
                        .orientation(ScrollbarOrientation::VerticalLeft)
                        .begin_symbol(Some("↑"))
                        .end_symbol(Some("↓")),
                    f.size().inner(&Margin { vertical: 10, horizontal: 10 }),
                    &mut self.popup_scroll_state,
                );
            }
            //f.render_widget(block, area);
        }

        Ok(())
    }
}
