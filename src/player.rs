use std::cmp;
use termion::event::Key;
use tui::terminal::Frame;
use tui::backend::Backend;
use tui::widgets::{ Widget, Block, Borders, SelectableList, Gauge, BarChart, Paragraph, Text };
use tui::style::{ Color, Modifier, Style};
use tui::layout::{ Rect, Layout, Constraint, Direction, Alignment };
use rfmod::Sys;
use super::{ Config, LyricsGrabber };

pub struct Song {
    pub name: String,
    pub path: String,
    pub artist: String,
    pub album: String,
    pub length: u32,
}

pub struct Playlist {
    pub songs: Vec<Song>,
    pub name: String,
    pub length: u32
}

pub trait Player {
    fn input(&mut self, key: Key, fmod: &Sys, media_player: &mut MediaPlayer);
    fn tick(&mut self, fmod: &Sys, media_player: &mut MediaPlayer);
    fn draw<B: Backend>(&mut self, f: &mut Frame<B>, chunk: Rect, media_player: &mut MediaPlayer);
}

pub struct MediaPlayer {
    pub num_spectrum_bars: usize,
    pub spectrum_data_last: Vec<f32>,
    pub last_song_title: Option<String>,
    pub playing_song_handle: Option<rfmod::Sound>,
    pub playing_channel: Option<rfmod::Channel>,
    pub playing_song_title: Option<String>,
    pub playing_song_lyrics: Option<String>,
    config: Config
}

impl MediaPlayer {
    pub fn new(config: Config) -> MediaPlayer {
          MediaPlayer {
              num_spectrum_bars: 70,
              spectrum_data_last: vec![0f32; 70],
              last_song_title: None,
              playing_song_handle: None,
              playing_channel: None,
              playing_song_title: None,
              playing_song_lyrics: None,
              config: config
          }
    }

    pub fn forward(&self) {
        self.playing_channel.as_ref().unwrap().set_position(cmp::max(0, self.playing_channel.as_ref().unwrap().get_position(rfmod::TIMEUNIT_MS).unwrap() as i32 - 10000) as usize, rfmod::TIMEUNIT_MS);
    }

    pub fn back(&self) {
        self.playing_channel.as_ref().unwrap().set_position(self.playing_channel.as_ref().unwrap().get_position(rfmod::TIMEUNIT_MS).unwrap() + 10000, rfmod::TIMEUNIT_MS);
    }

    pub fn almost_over(&self) -> bool {
        if let (Some(channel), Some(song_handle)) = (self.playing_channel.as_ref(), self.playing_song_handle.as_ref()) {
            if let (Ok(position_ms), Ok(song_length_ms)) = (channel.get_position(rfmod::TIMEUNIT_MS), song_handle.get_length(rfmod::TIMEUNIT_MS)) {
                return (position_ms as u32) >= (song_length_ms as u32) - 5
            }
        }
        true
    }

    pub fn pause(&self) {
        self.playing_channel.as_ref().unwrap().set_paused(true);
    }

    pub fn toggle_pause(&self) {
        self.playing_channel.as_ref().unwrap().set_paused(!self.playing_channel.as_ref().unwrap().get_paused().unwrap());
    }

    pub fn set_position(&self, loc: usize) {
        self.playing_channel.as_ref().unwrap().set_position(loc, rfmod::TIMEUNIT_MS);
    }

    pub fn play_from_uri(&mut self, fmod: &Sys, path: &str) {
        let playing_song_handle = match fmod.create_sound(path, None, None) {
            Ok(s) => s,
            Err(err) => panic!("Error code: {:?}", err)
        };
        let playing_channel = match playing_song_handle.play() {
            Ok(c) => c,
            Err(err) => panic!("Play: {:?}", err)
        };
        
        self.playing_song_handle = Some(playing_song_handle);
        self.playing_channel = Some(playing_channel);
        self.playing_song_lyrics = None;
    }

    pub fn draw<B: Backend>(&mut self, f: &mut Frame<B>, chunk: Rect, list_title: &str, list_member_titles: Vec<String>, selected_idx: usize, artist: String, album: String) {
        self.playing_song_title = Some(list_member_titles[selected_idx].clone());
        if let Some(playing_channel) = &mut self.playing_channel {
            if let Some(playing_song_handle) = &mut self.playing_song_handle {
                if self.playing_song_lyrics == None { // will try to grab every iteration if not found first time, replace this with something better future me, I have responsibilites rn
                    self.playing_song_lyrics = Some(String::new()); //LyricsGrabber::grab_lyrics(artist.clone(), self.playing_song_title.as_ref().unwrap().to_string(), &self.config.genius_token);
                }

                let chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints(vec![Constraint::Percentage(50), Constraint::Percentage(50)])
                    .split(chunk);
                let select_list_style = Style::default().fg(Color::White);
                SelectableList::default()
                    .block(Block::default().borders(Borders::ALL).title(list_title))
                    .items(&list_member_titles)
                    .select(Some(selected_idx))
                    .style(select_list_style)
                    .highlight_style(select_list_style.modifier(Modifier::BOLD))
                    .render(f, chunks[1]);
                
                let time_ms = playing_channel.get_position(rfmod::TIMEUNIT_MS).unwrap() as f32;
                let time_s = time_ms / 1000.0 % 60.0;
                let time_m = time_ms / 1000.0 / 60.0;
                let spectrum_data = &playing_channel.get_wave_data(self.num_spectrum_bars, 1).unwrap();
                let mut spectrum_tuples: Vec<(&str, u64)> = Vec::new();
                for (idx, &s) in spectrum_data.iter().enumerate() { 
                    let value = (self.spectrum_data_last[idx].abs() + s.abs()) / 2.0 * 100.0 + 2.0;
                    spectrum_tuples.push(("", value as u64)); 
                    self.spectrum_data_last[idx] = s;
                }
                
                let text_obj = format!("Artist: {}\nAlbum: {}", artist, album);
                let info_text = [
                    Text::raw(&text_obj),
                    Text::raw(match &self.playing_song_lyrics {
                        Some(lyrics) => lyrics,
                        None => ""
                    })
                ];
                
                let player_chunks = Layout::default()
                    .constraints([Constraint::Min(7), Constraint::Min(5), Constraint::Length(3)].as_ref())
                    .direction(Direction::Vertical)
                    .split(chunks[0]);
                BarChart::default()
                    .block(Block::default().borders(Borders::ALL))
                    .bar_width(1)
                    .bar_gap(1)
                    .style(Style::default().fg(Color::White))
                    .data(&spectrum_tuples)
                    .max(100)
                    .render(f, player_chunks[0]);
                Paragraph::new(info_text.iter())
                    .block(Block::default().title(&format!("{}{}", list_member_titles[selected_idx], if false { " PAUSED" } else { "" })).borders(Borders::ALL))
                    .alignment(Alignment::Left)
                    .render(f, player_chunks[1]);
                Gauge::default()
                    .block(Block::default().borders(Borders::ALL))
                    .style(Style::default().fg(Color::White))
                    .percent((time_ms / playing_song_handle.get_length(rfmod::TIMEUNIT_MS).unwrap() as f32 * 100.0) as u16)
                    .label(&format!("{}{}:{}{}", if time_m < 10.0 { "0" } else { "" }, time_m as u32, if time_s < 10.0 { "0" } else { "" }, time_s as u32))
                    .render(f, player_chunks[2]);
            }
        }
        self.last_song_title = self.playing_song_title.clone();
    }

    fn input(&mut self, _key: Key, _fmod: &Sys) {

    }

    fn tick(&mut self, _fmod: &Sys) {

    }
}