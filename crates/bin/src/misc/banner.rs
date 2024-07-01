use std::{
    collections::VecDeque,
    io::{stdout, Stdout, Write},
    thread,
    time::Duration,
};

use crossterm::{
    cursor::{Hide, MoveTo, Show},
    execute,
    style::Print,
    terminal::{size, Clear, ClearType},
};
use rand::{seq::SliceRandom, thread_rng, Rng};
use regex::Regex;

use super::art::BRONTES_BANNER;

pub fn rain() {
    let mut stdout = stdout();
    execute!(stdout, Hide, Clear(ClearType::FromCursorDown)).unwrap();

    let lines = initialize_positions(&mut stdout, &parse_ansi_text(BRONTES_BANNER));
    let mut active_lines = Vec::new();
    let mut animated_term = AnimatedTerm::new(stdout);

    let moves_before_next_starts = 5;
    let mut moves_count = 0;
    let final_coordinates = lines.first().unwrap().segments.first().unwrap().clone();

    for line in lines {
        active_lines.push(line);

        while moves_count < moves_before_next_starts {
            for (i, line) in active_lines.iter_mut().enumerate() {
                if !line.set {
                    let moves_budget = std::cmp::max(moves_before_next_starts - i as isize, 0);
                    for _ in 0..moves_budget {
                        if line.move_down(&mut animated_term) {
                            break;
                        }
                        thread::sleep(Duration::from_micros(25));
                    }
                }
            }
            moves_count += 1;
            animated_term.term.flush().unwrap();
        }

        moves_count = 0;
    }

    while !active_lines.is_empty() {
        let mut time_to_sleep = 25;
        let mut i = 0;
        while i < active_lines.len() {
            let moves_budget = std::cmp::max(12 - i as isize, 0);

            let mut moves_made = 0;
            while moves_made < moves_budget && !active_lines[i].set {
                if active_lines[i].move_down(&mut animated_term) {
                    break;
                }
                moves_made += 1;

                thread::sleep(Duration::from_micros(time_to_sleep));
            }

            if active_lines[i].set {
                active_lines.remove(i);
                time_to_sleep = 25 + 100 * i as u64;
            } else {
                i += 1;
            }
        }

        animated_term.term.flush().unwrap();
    }

    execute!(
        animated_term.term,
        MoveTo(final_coordinates.x as u16 + 20, (final_coordinates.final_y + 1) as u16),
        Print("\x1B[0m"),
        Show
    )
    .unwrap();
}

pub struct AnimatedTerm {
    pub term:       Stdout,
    pub term_state: Vec<Vec<bool>>,
}

impl AnimatedTerm {
    pub fn new(term: Stdout) -> Self {
        let term_state = vec![vec![false; 100]; 100];
        Self { term, term_state }
    }

    pub fn update_state(&mut self, x: usize, y: usize, state: bool, length: usize) {
        if y < self.term_state.len() {
            let end_x = std::cmp::min(x + length, self.term_state[y].len());
            for idx in x..end_x {
                self.term_state[y][idx] = state;
            }
        }
    }

    pub fn is_free(&self, x: usize, y: usize, length: usize) -> bool {
        if y < self.term_state.len() {
            let end_x = std::cmp::min(x + length, self.term_state[y].len());
            return !self.term_state[y][x..end_x].iter().any(|&state| state);
        }
        false
    }

    pub fn find_nearest_free_space(
        &self,
        x: usize,
        start_y: usize,
        final_y: usize,
        length: usize,
    ) -> Option<usize> {
        (start_y..=final_y).find(|&y| self.is_free(x, y, length))
    }
}
fn initialize_positions(
    term: &mut Stdout,
    parsed_banner: &[Vec<(String, usize)>],
) -> Vec<BannerLine> {
    let (_, y) = crossterm::cursor::position().expect("Failed to get cursor position");

    let (_, terminal_height) = size().expect("Failed to get terminal size");

    let banner_height = parsed_banner.len();
    let banner_h = banner_height as u16;
    let space_below_cursor = terminal_height - y;

    let needed_lines =
        if banner_h > space_below_cursor { (banner_h - space_below_cursor) as usize } else { 0 };

    if needed_lines > 0 {
        for _ in 0..needed_lines {
            execute!(term, Print("\n")).unwrap();
        }
    }

    let start_y = (y as usize).saturating_sub(banner_height);

    thread::sleep(Duration::from_secs(1));

    let mut banner_lines = Vec::new();

    for (i, line) in parsed_banner.iter().rev().enumerate() {
        let final_y = banner_height - i - 1 + start_y;
        let mut line_positions = Vec::new();
        let mut x = 0;
        for &(ref segment, len) in line {
            line_positions.push(Segment::new(segment.clone(), len, x, start_y, final_y));
            x += len;
        }
        let banner_line = BannerLine::new(line_positions, false);
        banner_lines.push(banner_line);
    }
    banner_lines.remove(0);
    banner_lines
}

pub struct BannerLine {
    pub segments: Vec<Segment>,
    pub set:      bool,
    pub inited:   bool,
}

impl BannerLine {
    pub fn new(segments: Vec<Segment>, set: bool) -> Self {
        Self { segments, set, inited: false }
    }

    pub fn move_down(&mut self, animated_term: &mut AnimatedTerm) -> bool {
        if !self.inited {
            let mut rng = thread_rng();
            self.segments.shuffle(&mut rng);
            for segment in &mut self.segments {
                segment.init(animated_term);
            }
            self.inited = true;
            false
        } else {
            let mut rng = thread_rng();
            self.segments.shuffle(&mut rng);

            let all_set = self
                .segments
                .iter_mut()
                .all(|segment| segment.move_down(animated_term));

            self.set = all_set;
            self.set
        }
    }
}

#[derive(Clone)]
pub struct Segment {
    pub text:     String,
    pub text_len: usize,
    pub x:        usize,
    pub y:        usize,
    pub final_y:  usize,
}

impl Segment {
    pub fn new(text: String, text_len: usize, x: usize, y: usize, final_y: usize) -> Self {
        Self { text, text_len, x, y, final_y }
    }

    pub fn init(&self, animated_term: &mut AnimatedTerm) {
        execute!(animated_term.term, MoveTo(self.x as u16, self.y as u16), Print(&self.text))
            .unwrap();
        animated_term.update_state(self.x, self.y, true, self.text_len);
    }

    pub fn move_down(&mut self, animated_term: &mut AnimatedTerm) -> bool {
        if self.y == self.final_y {
            return true;
        }

        let distance = rand::thread_rng()
            .gen_range(0..=3)
            .min(self.final_y - self.y);

        if animated_term.is_free(self.x, self.y + distance, self.text_len) {
            animated_term.update_state(self.x, self.y, false, self.text_len);
            execute!(
                animated_term.term,
                MoveTo(self.x as u16, self.y as u16),
                Print("\x1B[0m"),
                Print(" ".repeat(self.text_len)),
                MoveTo(self.x as u16, (self.y + distance) as u16),
                Print(&self.text)
            )
            .unwrap();
            self.y += distance;
            animated_term.update_state(self.x, self.y, true, self.text_len);
            self.y == self.final_y
        } else {
            let y = animated_term.find_nearest_free_space(
                self.x,
                self.y + 1,
                self.final_y,
                self.text_len,
            );

            if let Some(y) = y {
                animated_term.update_state(self.x, self.y, false, self.text_len);
                execute!(
                    animated_term.term,
                    MoveTo(self.x as u16, self.y as u16),
                    Print("\x1B[0m"),
                    Print(" ".repeat(self.text_len)),
                    MoveTo(self.x as u16, y as u16),
                    Print(&self.text)
                )
                .unwrap();
                self.y = y;
                animated_term.update_state(self.x, self.y, true, self.text_len);
                self.y == self.final_y
            } else {
                false
            }
        }
    }
}

fn parse_ansi_text(input: &str) -> Vec<Vec<(String, usize)>> {
    let re = Regex::new(r"(\x1b\[[0-9;]*m)").unwrap();
    let lines = input.split('\n');
    let mut parsed_output = Vec::new();

    for line in lines {
        let mut line_segments = Vec::new();
        let mut last_pos = 0;
        let mut segments = VecDeque::new();

        for cap in re.find_iter(line) {
            let start = cap.start();
            let end = cap.end();
            // Capture plain text before the ANSI sequence
            if start != last_pos {
                segments.push_back((String::new(), line[last_pos..start].to_string()));
            }
            // Capture the ANSI sequence
            segments.push_back((line[start..end].to_string(), String::new()));
            last_pos = end;
        }

        if last_pos < line.len() {
            segments.push_back((String::new(), line[last_pos..].to_string()));
        }

        let mut current_ansi = String::new();
        for (seq, text) in segments {
            if !seq.is_empty() {
                current_ansi = seq;
            }
            if !text.is_empty() {
                line_segments.push((format!("{}{}", current_ansi, text), text.len()));
            }
        }

        parsed_output.push(line_segments);
    }

    parsed_output
}

// Building Sorella
// ----------------
// Karthik
// Joseph
// Will Smith
// Ludwig
// 20-30k
//Office space: 25-30k
