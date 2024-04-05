use std::{collections::HashMap, time::Duration};

use color_eyre::eyre::Result;
use ratatui::{prelude::*, widgets::*};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;

use super::{Component, Frame};

  use brontes_types::mev::events::Action;

use crate::tui::app::AppContext;

use crate::tui::{
    colors::RgbSwatch,
    config::{Config, KeyBindings},
    //events::{Event, EventHandler},
    app::layout,
    theme::THEME,
    tui::Event,
};
use std::{

    sync::{Arc, Mutex},

};

use crossterm::event::{
    Event as CrosstermEvent, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent,
};

use tracing::info;
use brontes_database::libmdbx::implementation::compressed_wrappers::tx::CompressedLibmdbxTx;
use brontes_database::libmdbx::tables::*;
use brontes_database::libmdbx::Libmdbx;

use std::{
  env
};


#[derive(Clone,Debug, Default)]
pub struct DbSize {
  command_tx: Option<UnboundedSender<Action>>,
  config: Config,
  leaderboard:        Vec<(&'static str, u64)>,

}

impl DbSize{
    pub fn new() -> Self {
    Self { command_tx: Default::default(), config:  Default::default(),  leaderboard: vec![
        ("jaredfromsubway.eth", 1_200_000),
        ("0x23892382394..212", 1_100_000),
        ("0x13897682394..243", 1_000_000),
        ("0x33899882394..223", 900_000),
        ("0x43894082394..265", 800_000),
        ("0x53894082394..283", 700_000),
        ("0x83894082394..293", 600_000),
        // Repeat as necessary
    ], }
  }

  fn draw_dbsize(widget: &DbSize, area: Rect, buf: &mut Buffer) {


  // Construct the final Vec<(&str, u64)> with the total counts
  let data: Vec<(&str, u64)> = vec![
    ("Sandwich", 20),
    ("Cex-Dex", 19),
    ("Jit", 15),
    ("Jit Sandwich", 10),
    ("Atomic Backrun", 5),
    ("Liquidation", 3),
];

let barchart = BarChart::default()
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title("DB Size by tables"),
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


}

impl Component for DbSize {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
    self.command_tx = Some(tx);
    Ok(())
  }

  fn register_config_handler(&mut self, config: Config) -> Result<()> {
    self.config = config;
    Ok(())
  }
  fn name(&self) -> String {
    "DbSize".to_string()
  }



  fn update(&mut self, action: Action) -> Result<Option<Action>> {
    match action {
      Action::Tick => {
      },
      _ => {},
    }
    Ok(None)
  }



  fn init(&mut self, area: Rect) -> Result<()> {

  let brontes_db_endpoint = env::var("BRONTES_DB_PATH").expect("No BRONTES_DB_PATH in .env");
// TODO: Get tables and sizes


  Ok(())

}

  fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {

    let area = area.inner(&Margin { vertical: 1, horizontal: 4 });

    let template = Layout::default()
        .constraints([Constraint::Length(5), Constraint::Min(8), Constraint::Length(1)])
        .split(area);

    let chunks = Layout::default()
        .constraints([Constraint::Length(11), Constraint::Min(8), Constraint::Length(20)])
        .split(template[1]);

    let sub_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[0]);


    let buf = f.buffer_mut();

    Self::draw_dbsize(self, sub_layout[0], buf);

    Ok(())
  }
}

