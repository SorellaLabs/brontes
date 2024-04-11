#![allow(unused_variables)]

// Finish this file as a last thing to do
use brontes_database::tui::events::TuiUpdate;
use clap::Parser;
use color_eyre::eyre::Result;
use crossterm::event::{Event, KeyEvent};
use ratatui::{prelude::*, widgets::*};
use tokio::sync::mpsc::UnboundedSender;
use tracing::info;
use tui_textarea::TextArea;

use crate::{
    cli::{Args, Commands},
    tui::{
        components::{
            constants::UiStyle,
            ClickableList::{default_block, selectable_list, ClickableListState},
            Component, Frame,
        },
        config::Config,
    },
};
#[derive(Debug)]
pub struct Settings {
    command_tx:         Option<UnboundedSender<TuiUpdate>>,
    config:             Config,
    pub exchange_index: Option<usize>,
    state:              SettingsState,
}

#[derive(Debug, Default, PartialOrd, PartialEq)]
pub enum SettingsState {
    #[default]
    StartBlock,
    EndBlock,
    Inspectors,
    Exchanges,
    Done,
}

impl SettingsState {
    pub fn next(&self) -> Self {
        match self {
            SettingsState::StartBlock => SettingsState::EndBlock,
            SettingsState::EndBlock => SettingsState::Inspectors,
            SettingsState::Inspectors => SettingsState::Exchanges,
            SettingsState::Exchanges => SettingsState::Done,
            SettingsState::Done => SettingsState::Done,
        }
    }

    pub fn previous(&self) -> Self {
        match self {
            SettingsState::StartBlock => SettingsState::StartBlock,
            SettingsState::EndBlock => SettingsState::StartBlock,
            SettingsState::Inspectors => SettingsState::EndBlock,
            SettingsState::Exchanges => SettingsState::Inspectors,
            SettingsState::Done => SettingsState::Exchanges,
        }
    }
}

/*
    pub start_block:       Option<u64>,
    /// Optional End Block, if omitted it will run historically & at tip until
    /// killed
    #[arg(long, short)]
    pub end_block:         Option<u64>,
    /// Optional Max Tasks, if omitted it will default to 80% of the number of
    /// physical cores on your machine
    #[arg(long, short)]
    pub max_tasks:         Option<u64>,
    /// Optional minimum batch size
    #[arg(long, default_value = "500")]
    pub min_batch_size:    u64,
    /// Optional quote asset, if omitted it will default to USDT
    #[arg(long, short, default_value = USDT_ADDRESS_STRING)]
    pub quote_asset:       String,
    /// Inspectors to run. If omitted it defaults to running all inspectors
    #[arg(long, short, value_delimiter = ',')]
    pub inspectors:        Option<Vec<Inspectors>>,
    /// Centralized exchanges to consider for cex-dex inspector
    #[arg(long, short, default_values = &["Binance", "Coinbase", "Okex", "BybitSpot", "Kucoin"], value_delimiter = ',')]
    pub cex_exchanges:     Vec<String>,
    /// Ensures that dex prices are calcuated for every new block, even if the
    /// db already contains the price
    #[arg(long, short, default_value = "false")]
    pub force_dex_pricing: bool,
    /// How many blocks behind chain tip to run.
    #[arg(long, default_value = "3")]
    pub behind_tip:        u64,
}


*/

impl Settings {
    pub fn new() -> Self {
        let opt = Args::parse();
        info!("args: {:?}", opt);
        Self {
            command_tx:     Default::default(),
            config:         Default::default(),
            exchange_index: Default::default(),
            state:          Default::default(),
        }
    }

    fn inactivate(textarea: &mut TextArea<'_>) {
        textarea.set_cursor_line_style(Style::default());
        textarea.set_cursor_style(Style::default());
        textarea.set_block(
            Block::default()
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::DarkGray))
                .title(" Inactive (^X to switch) "),
        );
    }

    fn activate(textarea: &mut TextArea<'_>) {
        textarea.set_cursor_line_style(Style::default().add_modifier(Modifier::UNDERLINED));
        textarea.set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
        textarea.set_block(
            Block::default()
                .borders(Borders::ALL)
                .style(Style::default())
                .title(" Active "),
        );
    }

    pub fn set_state(&mut self, state: SettingsState) {
        self.state = state;
    }

    fn validate(textarea: &mut TextArea) -> bool {
        if let Err(err) = textarea.lines()[0].parse::<f64>() {
            textarea.set_style(Style::default().fg(Color::LightRed));
            textarea.set_block(
                Block::default().borders(Borders::ALL), // .title(format!("ERROR: {}", err)),
            );
            false
        } else {
            textarea.set_style(Style::default().fg(Color::LightGreen));
            textarea.set_block(Block::default().borders(Borders::ALL));
            true
        }
    }
}
impl Component for Settings {
    fn name(&self) -> String {
        "settings".to_string()
    }

    fn register_config_handler(&mut self, config: Config) -> Result<()> {
        self.config = config;
        Ok(())
    }

    fn handle_key_events(&mut self, key: KeyEvent) {
        //TODO: handle settings

        /*
                  SettingsState::EndBlock => SettingsState::Inspectors,
                  SettingsState::Inspectors => SettingsState::Exchanges,
                  SettingsState::Exchanges => SettingsState::Done,
                  SettingsState::Done => SettingsState::Done,


              match key.code {
                KeyCode::Up => self.state.previous(),
                KeyCode::Down => self.state.next(),
                _ => {},
        /*
                _ => {
                    match self.state {
                      SettingsState::StartBlock => match key.code {
                            KeyCode::Enter => {
                                self.set_state(self.state.next());
                            }
                            _ => {
                               //add_text
                               self.set_state(self.state.next());

                            }
                        },
                        SettingsState::EndBlock => match key.code {
                            KeyCode::Enter => {

                                self.set_state(self.state.next())
                            }
                            KeyCode::Backspace => {
                              self.set_state(self.state.next());

                            }
                            _ => {
                              self.set_state(self.state.next());
                            }
                        },
                        SettingsState::Inspectors => match key.code {
                            KeyCode::Enter => self.set_state(self.state.next()),
                            KeyCode::Backspace => {

                                self.set_state(self.state.previous());
                            }

                            _ => {
                              self.set_state(self.state.next());

                            }
                        },
                        SettingsState::Exchanges => match key.code {
                            KeyCode::Enter => {
                                self.set_state(self.state.next());
                            }
                            KeyCode::Backspace => {
                                self.set_state(self.state.previous());
                            }

                            _ => {
                              self.set_state(self.state.next());

                            }
                        },


                        SettingsState::Done => match key.code {
                            KeyCode::Enter => {
                              self.set_state(self.state.next());

                              /*
                                return Some(UiCallbackPreset::GeneratePlayerTeam {
                                    name: self.team_name_textarea.lines()[0].clone(),
                                    home_planet: self.planet_ids[self.planet_index].clone(),
                                    jersey_style: self.jersey_styles[self.jersey_style_index],
                                    jersey_colors: self.get_team_colors(),
                                    players: self.selected_players.clone(),
                                    balance: self.get_remaining_balance() as u32,
                                    spaceship: self.selected_ship().clone(),
                                });
                                */
                            }
                            KeyCode::Backspace => {
                              self.set_state(self.state.next());

                                //self.set_index(0);
                                //return Some(UiCallbackPreset::CancelGeneratePlayerTeam);
                            }
                            KeyCode::Left => {
                              self.set_state(self.state.next());

                               // self.confirm = ConfirmChoice::Yes;
                            }
                            KeyCode::Right => {
                              self.set_state(self.state.next());

                             //   self.confirm = ConfirmChoice::No;
                            }
                            _ => {
                              self.set_state(self.state.next());

                            }
                        },
                    }
                }
        */


            }
        */
    }

    fn handle_events(&mut self, event: Option<Event>) {
        match event {
            Some(Event::Key(key_event)) => self.handle_key_events(key_event),
            Some(Event::Mouse(mouse_event)) => self.handle_mouse_events(mouse_event),
            _ => (),
        }
    }

    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) {
        let area = area.inner(&Margin { vertical: 1, horizontal: 4 });

        let template = Layout::default()
            .constraints([Constraint::Length(1), Constraint::Min(8), Constraint::Length(1)])
            .split(area);

        let chunks = Layout::default()
            .constraints([Constraint::Length(9), Constraint::Min(8), Constraint::Length(20)])
            .split(template[1]);

        let sub_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[0]);

        let sub_sub_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(sub_layout[0]);

        let opt = Args::parse();
        let command_string = "";

        match opt.command {
            Commands::Run(command) => {
                // Now `run_args` is your `RunArgs` struct, and you can access its fields
                let mut textarea = TextArea::from([command.start_block.unwrap().to_string()]);
                textarea.set_cursor_line_style(Style::default());
                textarea.set_style(Style::default().fg(Color::LightGreen));

                textarea.set_block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(format!("Start Block")),
                );

                let mut textarea2 = TextArea::from([command.end_block.unwrap().to_string()]);
                textarea2.set_cursor_line_style(Style::default());
                textarea2.set_block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(format!("End Block")),
                );

                // let mut is_valid = Self::validate(&mut textarea);
                // let mut is_valid = Self::validate(&mut textarea2);
                f.render_widget(textarea.widget(), sub_sub_layout[0]);
                f.render_widget(textarea2.widget(), sub_sub_layout[1]);

                let list = selectable_list(vec![
                    ("test1".to_string(), UiStyle::DEFAULT),
                    ("test2".to_string(), UiStyle::DEFAULT),
                    ("test3".to_string(), UiStyle::DEFAULT),
                    ("test4".to_string(), UiStyle::DEFAULT),
                ]);

                let constraints = vec![Constraint::Length(10)].repeat(2);
                let rect = Rect { x: 4, y: 8, width: 20, height: 20 };
                let split = Layout::vertical(constraints).split(rect);

                f.render_stateful_widget(
                    list.block(default_block().title("Exchanges")),
                    split[1],
                    &mut ClickableListState::default().with_selected(self.exchange_index),
                );
            }
            _ => {}
        }
    }
}
