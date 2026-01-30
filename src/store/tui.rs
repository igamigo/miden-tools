//! Interactive TUI for browsing the miden-client store.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use miden_client::Serializable;
use miden_client::{
    Word,
    block::BlockHeader,
    crypto::{InOrderIndex, MmrPeaks},
    note::NoteHeader,
    store::{NoteFilter, PartialBlockchainFilter, Store, TransactionFilter},
};
use miden_client_sqlite_store::SqliteStore;
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Tabs, Wrap},
};
use rusqlite::{Connection, params};
use tokio::runtime::{Handle, Runtime};

use crate::render::note::{format_note_tag, well_known_label_from_root};

// ================================================================================================
// Tab enum
// ================================================================================================

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Accounts,
    InputNotes,
    OutputNotes,
    Transactions,
    Blocks,
}

impl Tab {
    fn all() -> [Tab; 5] {
        [
            Tab::Accounts,
            Tab::InputNotes,
            Tab::OutputNotes,
            Tab::Transactions,
            Tab::Blocks,
        ]
    }

    fn title(self) -> &'static str {
        match self {
            Tab::Accounts => "Accounts",
            Tab::InputNotes => "Input Notes",
            Tab::OutputNotes => "Output Notes",
            Tab::Transactions => "Transactions",
            Tab::Blocks => "Blocks",
        }
    }

    fn index(self) -> usize {
        match self {
            Tab::Accounts => 0,
            Tab::InputNotes => 1,
            Tab::OutputNotes => 2,
            Tab::Transactions => 3,
            Tab::Blocks => 4,
        }
    }
}

// ================================================================================================
// Entry point
// ================================================================================================

pub(crate) fn run_store_tui(store_path: PathBuf) -> Result<()> {
    let rt = Runtime::new()?;
    let _guard = rt.enter();
    let handle = rt.handle().clone();
    let store = handle
        .block_on(SqliteStore::new(store_path.clone()))
        .with_context(|| format!("failed to open store at {}", store_path.display()))?;

    let mut app = StoreTui::new(&store_path, store, handle)?;
    app.refresh_data()?;

    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = app.run(&mut terminal);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    drop(app);

    result
}

// ================================================================================================
// Main TUI struct
// ================================================================================================

struct StoreTui {
    store: SqliteStore,
    conn: Connection,
    handle: Handle,
    tab: usize,
    accounts: Vec<miden_client::account::AccountId>,
    account_headers: Vec<Option<miden_client::account::AccountHeader>>,
    input_notes: Vec<miden_client::store::InputNoteRecord>,
    output_notes: Vec<miden_client::store::OutputNoteRecord>,
    transactions: Vec<miden_client::transaction::TransactionRecord>,
    block_headers: Vec<BlockHeader>,
    mmr_nodes: BTreeMap<InOrderIndex, Word>,
    mmr_peaks: Option<MmrPeaks>,
    input_notes_per_block: HashMap<u32, usize>,
    output_notes_per_block: HashMap<u32, usize>,
    output_notes_total: u64,
    output_notes_loaded: bool,
    selected: [usize; 5],
    detail_scroll: usize,
    filters: Vec<String>,
    visible: Vec<Vec<usize>>,
    filter_mode: bool,
    filter_input: String,
    status: String,
}

impl StoreTui {
    fn new(store_path: &PathBuf, store: SqliteStore, handle: Handle) -> Result<Self> {
        let conn = Connection::open(store_path)
            .with_context(|| format!("failed to open store at {}", store_path.display()))?;
        Ok(Self {
            store,
            conn,
            handle,
            tab: 0,
            accounts: Vec::new(),
            account_headers: Vec::new(),
            input_notes: Vec::new(),
            output_notes: Vec::new(),
            transactions: Vec::new(),
            block_headers: Vec::new(),
            mmr_nodes: BTreeMap::new(),
            mmr_peaks: None,
            input_notes_per_block: HashMap::new(),
            output_notes_per_block: HashMap::new(),
            output_notes_total: 0,
            output_notes_loaded: false,
            selected: [0, 0, 0, 0, 0],
            detail_scroll: 0,
            filters: vec![String::new(); Tab::all().len()],
            visible: vec![Vec::new(); Tab::all().len()],
            filter_mode: false,
            filter_input: String::new(),
            status: String::new(),
        })
    }

    fn refresh_data(&mut self) -> Result<()> {
        let accounts = self.handle.block_on(self.store.get_account_ids())?;
        let mut headers = Vec::with_capacity(accounts.len());
        for account_id in &accounts {
            let header = self
                .handle
                .block_on(self.store.get_account_header(*account_id))?
                .map(|(header, _)| header);
            headers.push(header);
        }
        self.accounts = accounts;
        self.account_headers = headers;

        self.input_notes = self
            .handle
            .block_on(self.store.get_input_notes(NoteFilter::All))?;
        self.output_notes_total = query_u64(&self.conn, "SELECT COUNT(*) FROM output_notes")?;
        self.output_notes.clear();
        self.output_notes_loaded = false;
        self.transactions = self
            .handle
            .block_on(self.store.get_transactions(TransactionFilter::All))?;
        self.block_headers = self
            .handle
            .block_on(self.store.get_tracked_block_headers())?;
        self.block_headers.sort_by_key(|header| header.block_num());

        self.mmr_nodes = self.handle.block_on(
            self.store
                .get_partial_blockchain_nodes(PartialBlockchainFilter::All),
        )?;
        self.mmr_peaks = self.block_headers.last().and_then(|header| {
            self.handle
                .block_on(
                    self.store
                        .get_partial_blockchain_peaks_by_block_num(header.block_num()),
                )
                .ok()
        });

        self.input_notes_per_block = count_notes_per_block(&self.input_notes);
        self.output_notes_per_block.clear();

        self.update_status();
        self.rebuild_visible_all();

        Ok(())
    }

    fn update_status(&mut self) {
        self.status = format!(
            "accounts: {} | input: {} | output: {}/{} | txs: {} | blocks: {}",
            self.accounts.len(),
            self.input_notes.len(),
            self.output_notes.len(),
            self.output_notes_total,
            self.transactions.len(),
            self.block_headers.len()
        );
    }

    fn run(&mut self, terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>) -> Result<()> {
        loop {
            terminal.draw(|frame| self.render(frame))?;

            if event::poll(Duration::from_millis(200))? {
                if let Event::Key(key) = event::read()? {
                    if self.handle_key(key)? {
                        break;
                    }
                }
            }
        }
        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<bool> {
        if self.filter_mode {
            self.handle_filter_key(key);
            return Ok(false);
        }
        match key.code {
            KeyCode::Char('q') => return Ok(true),
            KeyCode::Char('r') => {
                if let Err(err) = self.refresh_data() {
                    self.status = format!("refresh failed: {err}");
                }
            }
            KeyCode::Left | KeyCode::Char('h') => self.prev_tab(),
            KeyCode::Right | KeyCode::Char('l') => self.next_tab(),
            KeyCode::Tab => self.next_tab(),
            KeyCode::BackTab => self.prev_tab(),
            KeyCode::Up | KeyCode::Char('k') => {
                self.move_selection(-1);
                self.detail_scroll = 0;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.move_selection(1);
                self.detail_scroll = 0;
            }
            KeyCode::Char('/') => {
                self.filter_mode = true;
                self.filter_input = self.filters[self.tab].clone();
            }
            // Navigation keys
            KeyCode::Char('b') => self.navigate_to_block(),
            KeyCode::Char('t') => self.navigate_to_transaction(),
            KeyCode::Char('n') => self.navigate_to_note(),
            KeyCode::Char('a') => self.navigate_to_account(),
            KeyCode::Enter => self.navigate_enter(),
            // Detail scroll
            KeyCode::PageDown | KeyCode::Char('J') => {
                self.detail_scroll = self.detail_scroll.saturating_add(5);
            }
            KeyCode::PageUp | KeyCode::Char('K') => {
                self.detail_scroll = self.detail_scroll.saturating_sub(5);
            }
            _ => {}
        }
        Ok(false)
    }

    fn handle_filter_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.filter_mode = false;
                self.filter_input.clear();
            }
            KeyCode::Enter => {
                self.filters[self.tab] = self.filter_input.trim().to_string();
                self.filter_mode = false;
                self.filter_input.clear();
                self.rebuild_visible(self.tab);
            }
            KeyCode::Backspace => {
                self.filter_input.pop();
            }
            KeyCode::Char(ch) => {
                if !ch.is_control() {
                    self.filter_input.push(ch);
                }
            }
            _ => {}
        }
    }

    // ============================================================================================
    // Navigation
    // ============================================================================================

    fn navigate_to_block(&mut self) {
        let block_num = match self.current_tab() {
            Tab::Transactions => {
                let Some(idx) = self.current_index() else {
                    return;
                };
                let tx = &self.transactions[idx];
                Some(tx.details.block_num.as_u32())
            }
            Tab::InputNotes => {
                let Some(idx) = self.current_index() else {
                    return;
                };
                let note = &self.input_notes[idx];
                note.inclusion_proof()
                    .map(|p| p.location().block_num().as_u32())
            }
            Tab::OutputNotes => {
                let Some(idx) = self.current_index() else {
                    return;
                };
                let note = &self.output_notes[idx];
                note.inclusion_proof()
                    .map(|p| p.location().block_num().as_u32())
            }
            _ => None,
        };

        if let Some(block_num) = block_num {
            self.jump_to_block(block_num);
        }
    }

    fn navigate_to_transaction(&mut self) {
        match self.current_tab() {
            Tab::InputNotes => {
                // Try to find transaction that consumed this note
                let Some(idx) = self.current_index() else {
                    return;
                };
                let note = &self.input_notes[idx];
                // Check if we can get consumer transaction from state
                if let Some(tx_id) = self.get_input_note_consumer_tx(note) {
                    self.jump_to_transaction_by_id(&tx_id);
                }
            }
            _ => {}
        }
    }

    fn navigate_to_note(&mut self) {
        match self.current_tab() {
            Tab::Transactions => {
                // Navigate to first input note of this transaction
                let Some(idx) = self.current_index() else {
                    return;
                };
                let tx = &self.transactions[idx];
                if let Some(nullifier_word) = tx.details.input_note_nullifiers.first() {
                    // Find input note with this nullifier
                    let target_nullifier = miden_client::note::Nullifier::from_raw(*nullifier_word);
                    for (i, note) in self.input_notes.iter().enumerate() {
                        if note.nullifier() == target_nullifier {
                            self.jump_to_input_note(i);
                            return;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn navigate_to_account(&mut self) {
        let account_id = match self.current_tab() {
            Tab::Transactions => {
                let Some(idx) = self.current_index() else {
                    return;
                };
                let tx = &self.transactions[idx];
                Some(tx.details.account_id)
            }
            Tab::InputNotes => {
                let Some(idx) = self.current_index() else {
                    return;
                };
                let note = &self.input_notes[idx];
                note.metadata().map(|m| m.sender())
            }
            Tab::OutputNotes => {
                let Some(idx) = self.current_index() else {
                    return;
                };
                let note = &self.output_notes[idx];
                Some(note.metadata().sender())
            }
            _ => None,
        };

        if let Some(account_id) = account_id {
            self.jump_to_account(account_id);
        }
    }

    fn navigate_enter(&mut self) {
        // Context-dependent navigation on Enter
        match self.current_tab() {
            Tab::Transactions => self.navigate_to_block(),
            Tab::InputNotes | Tab::OutputNotes => self.navigate_to_block(),
            Tab::Blocks => {
                // Could show notes in this block
            }
            _ => {}
        }
    }

    fn get_input_note_consumer_tx(
        &self,
        note: &miden_client::store::InputNoteRecord,
    ) -> Option<String> {
        // Check the note state to see if it has consumer transaction info
        use miden_client::store::InputNoteState;
        match note.state() {
            InputNoteState::ProcessingAuthenticated(state) => {
                Some(state.submission_data.consumer_transaction.to_string())
            }
            InputNoteState::ProcessingUnauthenticated(state) => {
                Some(state.submission_data.consumer_transaction.to_string())
            }
            InputNoteState::ConsumedAuthenticatedLocal(state) => {
                Some(state.submission_data.consumer_transaction.to_string())
            }
            InputNoteState::ConsumedUnauthenticatedLocal(state) => {
                Some(state.submission_data.consumer_transaction.to_string())
            }
            _ => None,
        }
    }

    fn jump_to_block(&mut self, block_num: u32) {
        // Find block index
        if let Some(pos) = self
            .block_headers
            .iter()
            .position(|h| h.block_num().as_u32() == block_num)
        {
            self.tab = Tab::Blocks.index();
            self.rebuild_visible(self.tab);
            // Find position in visible list
            if let Some(visible_pos) = self.visible[self.tab].iter().position(|&i| i == pos) {
                self.selected[self.tab] = visible_pos;
            }
            self.detail_scroll = 0;
        } else {
            self.status = format!("block {} not in store", block_num);
        }
    }

    fn jump_to_transaction_by_id(&mut self, tx_id: &str) {
        if let Some(pos) = self
            .transactions
            .iter()
            .position(|t| t.id.to_string() == tx_id)
        {
            self.tab = Tab::Transactions.index();
            self.rebuild_visible(self.tab);
            if let Some(visible_pos) = self.visible[self.tab].iter().position(|&i| i == pos) {
                self.selected[self.tab] = visible_pos;
            }
            self.detail_scroll = 0;
        }
    }

    fn jump_to_input_note(&mut self, pos: usize) {
        self.tab = Tab::InputNotes.index();
        self.rebuild_visible(self.tab);
        if let Some(visible_pos) = self.visible[self.tab].iter().position(|&i| i == pos) {
            self.selected[self.tab] = visible_pos;
        }
        self.detail_scroll = 0;
    }

    fn jump_to_account(&mut self, account_id: miden_client::account::AccountId) {
        if let Some(pos) = self.accounts.iter().position(|&a| a == account_id) {
            self.tab = Tab::Accounts.index();
            self.rebuild_visible(self.tab);
            if let Some(visible_pos) = self.visible[self.tab].iter().position(|&i| i == pos) {
                self.selected[self.tab] = visible_pos;
            }
            self.detail_scroll = 0;
        } else {
            self.status = format!("account {} not in store", account_id);
        }
    }

    // ============================================================================================
    // Tab navigation
    // ============================================================================================

    fn prev_tab(&mut self) {
        if self.tab == 0 {
            self.tab = Tab::all().len() - 1;
        } else {
            self.tab -= 1;
        }
        if self.current_tab() == Tab::OutputNotes {
            if let Err(err) = self.ensure_output_notes_loaded() {
                self.status = format!("output notes load failed: {err}");
            }
        }
        self.rebuild_visible(self.tab);
        self.detail_scroll = 0;
    }

    fn next_tab(&mut self) {
        self.tab = (self.tab + 1) % Tab::all().len();
        if self.current_tab() == Tab::OutputNotes {
            if let Err(err) = self.ensure_output_notes_loaded() {
                self.status = format!("output notes load failed: {err}");
            }
        }
        self.rebuild_visible(self.tab);
        self.detail_scroll = 0;
    }

    fn move_selection(&mut self, delta: isize) {
        let len = self.visible_len() as isize;
        if len == 0 {
            self.selected[self.tab] = 0;
            return;
        }

        let mut index = self.selected[self.tab] as isize + delta;
        if index < 0 {
            index = 0;
        } else if index >= len {
            index = len - 1;
        }
        self.selected[self.tab] = index as usize;
    }

    fn visible_len(&self) -> usize {
        self.visible
            .get(self.tab)
            .map(|list| list.len())
            .unwrap_or(0)
    }

    fn current_tab(&self) -> Tab {
        Tab::all()[self.tab]
    }

    fn current_index(&self) -> Option<usize> {
        self.visible
            .get(self.tab)
            .and_then(|list| list.get(self.selected[self.tab]).copied())
    }

    fn rebuild_visible_all(&mut self) {
        for idx in 0..Tab::all().len() {
            self.rebuild_visible(idx);
        }
    }

    fn rebuild_visible(&mut self, tab_index: usize) {
        let filter = self
            .filters
            .get(tab_index)
            .map(|f| f.to_lowercase())
            .unwrap_or_default();
        let matches = |text: &str| {
            if filter.is_empty() {
                true
            } else {
                text.to_lowercase().contains(&filter)
            }
        };

        let indices: Vec<usize> = match Tab::all()[tab_index] {
            Tab::Accounts => self
                .accounts
                .iter()
                .enumerate()
                .filter(|(_, id)| matches(&id.to_string()))
                .map(|(idx, _)| idx)
                .collect(),
            Tab::InputNotes => self
                .input_notes
                .iter()
                .enumerate()
                .filter(|(_, note)| matches(&note.id().to_string()))
                .map(|(idx, _)| idx)
                .collect(),
            Tab::OutputNotes => self
                .output_notes
                .iter()
                .enumerate()
                .filter(|(_, note)| matches(&note.id().to_string()))
                .map(|(idx, _)| idx)
                .collect(),
            Tab::Transactions => self
                .transactions
                .iter()
                .enumerate()
                .filter(|(_, tx)| matches(&tx.id.to_string()))
                .map(|(idx, _)| idx)
                .collect(),
            Tab::Blocks => self
                .block_headers
                .iter()
                .enumerate()
                .filter(|(_, header)| matches(&header.block_num().as_u32().to_string()))
                .map(|(idx, _)| idx)
                .collect(),
        };

        if self.visible.len() <= tab_index {
            self.visible.resize(tab_index + 1, Vec::new());
        }
        self.visible[tab_index] = indices;
        if self.selected[tab_index] >= self.visible[tab_index].len() {
            self.selected[tab_index] = self.visible[tab_index].len().saturating_sub(1);
        }
    }

    // ============================================================================================
    // Rendering
    // ============================================================================================

    fn render(&mut self, frame: &mut Frame) {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(5),
                Constraint::Length(3),
            ])
            .split(frame.size());

        let tab_titles: Vec<Line> = Tab::all()
            .iter()
            .map(|tab| Line::from(Span::styled(tab.title(), Style::default())))
            .collect();
        let tabs = Tabs::new(tab_titles)
            .select(self.tab)
            .block(Block::default().borders(Borders::ALL).title("Store TUI"))
            .highlight_style(Style::default().fg(Color::Yellow));
        frame.render_widget(tabs, layout[0]);

        let body = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(layout[1]);

        let mut list_state = ListState::default();
        let selection = if self.visible_len() == 0 {
            None
        } else {
            Some(self.selected[self.tab])
        };
        list_state.select(selection);
        let list = List::new(self.current_list_items())
            .block(Block::default().borders(Borders::ALL).title("Entries"))
            .highlight_style(Style::default().fg(Color::Cyan))
            .highlight_symbol(">> ");
        frame.render_stateful_widget(list, body[0], &mut list_state);

        let detail_lines = self.current_detail_lines();
        let nav_hints = self.navigation_hints();
        let title = if nav_hints.is_empty() {
            "Details".to_string()
        } else {
            format!("Details [{}]", nav_hints)
        };

        let details = Paragraph::new(detail_lines)
            .block(Block::default().borders(Borders::ALL).title(title))
            .wrap(Wrap { trim: false })
            .scroll((self.detail_scroll as u16, 0));
        frame.render_widget(details, body[1]);

        let footer = Paragraph::new(Line::from(self.status_line()))
            .style(Style::default().fg(Color::White))
            .block(Block::default().borders(Borders::ALL).title("Status"));
        frame.render_widget(footer, layout[2]);
    }

    fn navigation_hints(&self) -> String {
        match self.current_tab() {
            Tab::Transactions => "b:block n:note a:account".to_string(),
            Tab::InputNotes => "b:block t:tx a:sender".to_string(),
            Tab::OutputNotes => "b:block a:sender".to_string(),
            Tab::Blocks => String::new(),
            Tab::Accounts => String::new(),
        }
    }

    fn status_line(&self) -> String {
        let hint = "/ filter  r refresh  q quit  hjkl/arrows nav  PgUp/Dn scroll";
        let base = if self.status.is_empty() {
            hint.to_string()
        } else {
            format!("{} | {}", self.status, hint)
        };

        if self.filter_mode {
            format!("filter: {}_", self.filter_input)
        } else if !self.filters[self.tab].is_empty() {
            format!("{} | filter: {}", base, self.filters[self.tab])
        } else {
            base
        }
    }

    fn current_list_items(&self) -> Vec<ListItem<'static>> {
        let indices = self.visible.get(self.tab).cloned().unwrap_or_default();
        match self.current_tab() {
            Tab::Accounts => indices
                .into_iter()
                .map(|idx| ListItem::new(self.accounts[idx].to_string()))
                .collect(),
            Tab::InputNotes => indices
                .into_iter()
                .map(|idx| {
                    let note = &self.input_notes[idx];
                    let state_char = input_note_state_char(note);
                    ListItem::new(format!("[{}] {}", state_char, note.id()))
                })
                .collect(),
            Tab::OutputNotes => {
                if !self.output_notes_loaded {
                    return vec![ListItem::new("(loading...)")];
                }
                indices
                    .into_iter()
                    .map(|idx| {
                        let note = &self.output_notes[idx];
                        let state_char = output_note_state_char(note);
                        ListItem::new(format!("[{}] {}", state_char, note.id()))
                    })
                    .collect()
            }
            Tab::Transactions => indices
                .into_iter()
                .map(|idx| {
                    let tx = &self.transactions[idx];
                    let status_char = tx_status_char(tx);
                    ListItem::new(format!("[{}] {}", status_char, tx.id))
                })
                .collect(),
            Tab::Blocks => indices
                .into_iter()
                .map(|idx| {
                    let header = &self.block_headers[idx];
                    ListItem::new(format!("block {}", header.block_num().as_u32()))
                })
                .collect(),
        }
    }

    fn current_detail_lines(&self) -> Vec<Line<'static>> {
        if self.current_tab() == Tab::OutputNotes && !self.output_notes_loaded {
            return vec![Line::from("loading output notes...")];
        }

        let Some(idx) = self.current_index() else {
            return vec![Line::from("no selection")];
        };

        match self.current_tab() {
            Tab::Accounts => self.account_detail(idx),
            Tab::InputNotes => self.input_note_detail(idx),
            Tab::OutputNotes => self.output_note_detail(idx),
            Tab::Transactions => self.transaction_detail(idx),
            Tab::Blocks => self.block_detail(idx),
        }
    }

    // ============================================================================================
    // Detail renderers
    // ============================================================================================

    fn account_detail(&self, idx: usize) -> Vec<Line<'static>> {
        let id = self.accounts[idx];
        let mut lines = vec![];

        lines.push(label_line("account id", &id.to_string()));
        if let Some(header) = self.account_headers.get(idx).and_then(|h| h.clone()) {
            lines.push(label_line("nonce", &header.nonce().to_string()));
            lines.extend(hash_lines("vault", &header.vault_root().to_string()));
            lines.extend(hash_lines(
                "storage",
                &header.storage_commitment().to_string(),
            ));
            lines.extend(hash_lines("code", &header.code_commitment().to_string()));
        } else {
            lines.push(label_line("header", "n/a"));
        }

        match self.account_history(id) {
            Ok(history) => {
                lines.push(Line::from(""));
                lines.push(label_line("states in store", &history.total.to_string()));
                if !history.rows.is_empty() {
                    lines.push(Line::from(Span::styled(
                        "recent states:",
                        Style::default().add_modifier(Modifier::DIM),
                    )));
                    for row in history.rows {
                        lines.push(Line::from(format!(
                            "  nonce {} -> {}",
                            row.nonce, row.account_commitment
                        )));
                    }
                }
            }
            Err(err) => {
                lines.push(label_line("history", &format!("error: {err}")));
            }
        }
        lines
    }

    fn input_note_detail(&self, idx: usize) -> Vec<Line<'static>> {
        let note = &self.input_notes[idx];
        let details = note.details();
        let script_root = details.script().root();
        let script_label = well_known_label_from_root(&script_root);

        let mut lines = vec![];
        lines.extend(hash_lines("id", &note.id().to_string()));
        lines.push(label_line("state", &format_input_note_state(note)));

        if let Some(metadata) = note.metadata() {
            lines.push(label_line("sender", &metadata.sender().to_string()));
            lines.push(label_line("type", &format!("{:?}", metadata.note_type())));
            lines.push(label_line("tag", &format_note_tag(metadata.tag())));
        }

        lines.push(Line::from(""));
        if let Some(label) = script_label {
            lines.push(label_line(
                "script",
                &format!("{} ({})", label, script_root),
            ));
        } else {
            lines.extend(hash_lines("script root", &script_root.to_string()));
        }

        if let Some(commitment) = note.commitment() {
            lines.extend(hash_lines("commitment", &commitment.to_string()));
        }

        lines.push(label_line(
            "assets",
            &details.assets().num_assets().to_string(),
        ));
        lines.push(label_line(
            "inputs",
            &details.inputs().values().len().to_string(),
        ));

        // Inclusion info
        if let Some(proof) = note.inclusion_proof() {
            lines.push(Line::from(""));
            lines.push(label_line(
                "block",
                &proof.location().block_num().as_u32().to_string(),
            ));
            lines.push(label_line(
                "index",
                &proof.location().node_index_in_block().to_string(),
            ));
        }

        // Consumer transaction
        if let Some(tx_id) = self.get_input_note_consumer_tx(note) {
            lines.push(Line::from(""));
            lines.extend(hash_lines("consumer tx", &tx_id));
        }

        lines
    }

    fn output_note_detail(&self, idx: usize) -> Vec<Line<'static>> {
        let note = &self.output_notes[idx];
        let mut lines = vec![];

        lines.extend(hash_lines("id", &note.id().to_string()));
        lines.push(label_line("state", &format_output_note_state(note)));
        lines.push(label_line(
            "expected height",
            &note.expected_height().as_u32().to_string(),
        ));
        lines.push(label_line("sender", &note.metadata().sender().to_string()));
        lines.push(label_line(
            "type",
            &format!("{:?}", note.metadata().note_type()),
        ));
        lines.push(label_line("tag", &format_note_tag(note.metadata().tag())));
        lines.push(label_line(
            "assets",
            &note.assets().num_assets().to_string(),
        ));

        let commitment = NoteHeader::new(note.id(), note.metadata().clone()).commitment();
        lines.extend(hash_lines("commitment", &commitment.to_string()));

        if let Some(recipient) = note.recipient() {
            let script_root = recipient.script().root();
            let script_label = well_known_label_from_root(&script_root);
            lines.push(Line::from(""));
            if let Some(label) = script_label {
                lines.push(label_line(
                    "script",
                    &format!("{} ({})", label, script_root),
                ));
            } else {
                lines.extend(hash_lines("script root", &script_root.to_string()));
            }
            lines.push(label_line(
                "inputs",
                &recipient.inputs().values().len().to_string(),
            ));
        }

        // Inclusion info
        if let Some(proof) = note.inclusion_proof() {
            lines.push(Line::from(""));
            lines.push(label_line(
                "block",
                &proof.location().block_num().as_u32().to_string(),
            ));
            lines.push(label_line(
                "index",
                &proof.location().node_index_in_block().to_string(),
            ));
        }

        lines
    }

    fn transaction_detail(&self, idx: usize) -> Vec<Line<'static>> {
        let tx = &self.transactions[idx];
        let details = &tx.details;
        let mut lines = vec![];

        lines.extend(hash_lines("id", &tx.id.to_string()));
        lines.push(label_line("status", &tx.status.to_string()));
        lines.push(label_line("account", &details.account_id.to_string()));
        lines.push(label_line("block", &details.block_num.as_u32().to_string()));
        lines.push(label_line(
            "submission",
            &details.submission_height.as_u32().to_string(),
        ));
        lines.push(label_line(
            "expiration",
            &details.expiration_block_num.as_u32().to_string(),
        ));

        lines.push(Line::from(""));
        lines.push(label_line(
            "input nullifiers",
            &details.input_note_nullifiers.len().to_string(),
        ));
        for (i, nullifier) in details.input_note_nullifiers.iter().enumerate() {
            lines.extend(hash_lines(&format!("  [{}]", i), &nullifier.to_string()));
        }

        lines.push(label_line(
            "output notes",
            &details.output_notes.num_notes().to_string(),
        ));

        lines
    }

    fn block_detail(&self, idx: usize) -> Vec<Line<'static>> {
        let header = &self.block_headers[idx];
        let block_num = header.block_num().as_u32();
        let mut lines = vec![];

        lines.push(label_line("block", &block_num.to_string()));
        lines.push(label_line("timestamp", &header.timestamp().to_string()));
        lines.push(Line::from(""));

        lines.extend(hash_lines("commitment", &header.commitment().to_string()));
        lines.extend(hash_lines("chain", &header.chain_commitment().to_string()));
        lines.extend(hash_lines(
            "prev block",
            &header.prev_block_commitment().to_string(),
        ));
        lines.extend(hash_lines(
            "account root",
            &header.account_root().to_string(),
        ));
        lines.extend(hash_lines(
            "nullifier root",
            &header.nullifier_root().to_string(),
        ));
        lines.extend(hash_lines("note root", &header.note_root().to_string()));
        lines.extend(hash_lines(
            "tx commitment",
            &header.tx_commitment().to_string(),
        ));

        let input_count = self
            .input_notes_per_block
            .get(&block_num)
            .copied()
            .unwrap_or(0);
        let output_count = self.output_notes_per_block.get(&block_num).copied();
        lines.push(Line::from(""));
        lines.push(label_line("input notes", &input_count.to_string()));
        lines.push(label_line(
            "output notes",
            &output_count
                .map(|c| c.to_string())
                .unwrap_or_else(|| "n/a".to_string()),
        ));

        lines.push(Line::from(""));
        lines.push(label_line("mmr nodes", &self.mmr_nodes.len().to_string()));
        if let Some(peaks) = &self.mmr_peaks {
            lines.push(label_line("mmr peaks", &peaks.num_peaks().to_string()));
            lines.push(label_line("mmr leaves", &peaks.num_leaves().to_string()));
        }

        lines
    }

    fn ensure_output_notes_loaded(&mut self) -> Result<()> {
        if self.output_notes_loaded {
            return Ok(());
        }

        self.output_notes = self
            .handle
            .block_on(self.store.get_output_notes(NoteFilter::All))?;
        self.output_notes_per_block = count_notes_per_block(&self.output_notes);
        self.output_notes_loaded = true;
        self.update_status();
        self.rebuild_visible(self.tab);
        Ok(())
    }

    fn account_history(
        &self,
        account_id: miden_client::account::AccountId,
    ) -> Result<AccountHistory> {
        // Account IDs are stored as BLOBs in the sqlite store
        let id_bytes = account_id.to_bytes();

        let total: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM accounts WHERE id = ?",
                params![id_bytes],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let mut stmt = self.conn.prepare(
            "SELECT nonce, account_commitment FROM accounts WHERE id = ? ORDER BY nonce DESC LIMIT 5",
        )?;
        let rows = stmt.query_map(params![id_bytes], |row| {
            Ok(AccountHistoryRow {
                nonce: row.get(0)?,
                account_commitment: row.get(1)?,
            })
        })?;

        let mut history = Vec::new();
        for row in rows {
            history.push(row?);
        }

        Ok(AccountHistory {
            total: total as u64,
            rows: history,
        })
    }
}

// ================================================================================================
// Helper types
// ================================================================================================

struct AccountHistoryRow {
    nonce: i64,
    account_commitment: String,
}

struct AccountHistory {
    total: u64,
    rows: Vec<AccountHistoryRow>,
}

// ================================================================================================
// Formatting helpers
// ================================================================================================

/// Create a label: value line with the label dimmed.
fn label_line(label: &str, value: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("{}: ", label),
            Style::default().add_modifier(Modifier::DIM),
        ),
        Span::raw(value.to_string()),
    ])
}

/// Create lines for a hash value, putting it on a new line if the label is long.
fn hash_lines(label: &str, hash: &str) -> Vec<Line<'static>> {
    // Just put hash on same line - the Paragraph will wrap it
    vec![Line::from(vec![
        Span::styled(
            format!("{}: ", label),
            Style::default().add_modifier(Modifier::DIM),
        ),
        Span::raw(hash.to_string()),
    ])]
}

fn input_note_state_char(note: &miden_client::store::InputNoteRecord) -> char {
    use miden_client::store::InputNoteState;
    match note.state() {
        InputNoteState::Expected(_) => 'E',
        InputNoteState::Unverified(_) => 'U',
        InputNoteState::Committed(_) => 'C',
        InputNoteState::Invalid(_) => '!',
        InputNoteState::ProcessingAuthenticated(_) => 'P',
        InputNoteState::ProcessingUnauthenticated(_) => 'p',
        InputNoteState::ConsumedAuthenticatedLocal(_) => 'X',
        InputNoteState::ConsumedUnauthenticatedLocal(_) => 'x',
        InputNoteState::ConsumedExternal(_) => '*',
    }
}

fn output_note_state_char(note: &miden_client::store::OutputNoteRecord) -> char {
    use miden_client::store::OutputNoteState;
    match note.state() {
        OutputNoteState::ExpectedPartial => 'e',
        OutputNoteState::ExpectedFull { .. } => 'E',
        OutputNoteState::CommittedPartial { .. } => 'c',
        OutputNoteState::CommittedFull { .. } => 'C',
        OutputNoteState::Consumed { .. } => 'X',
    }
}

fn tx_status_char(tx: &miden_client::transaction::TransactionRecord) -> char {
    use miden_client::transaction::TransactionStatusVariant;
    match tx.status.variant() {
        TransactionStatusVariant::Pending => 'P',
        TransactionStatusVariant::Committed => 'C',
        TransactionStatusVariant::Discarded => 'D',
    }
}

fn format_input_note_state(note: &miden_client::store::InputNoteRecord) -> String {
    use miden_client::store::InputNoteState;
    match note.state() {
        InputNoteState::Expected(_) => "Expected".to_string(),
        InputNoteState::Unverified(_) => "Unverified".to_string(),
        InputNoteState::Committed(_) => "Committed".to_string(),
        InputNoteState::Invalid(_) => "Invalid".to_string(),
        InputNoteState::ProcessingAuthenticated(_) => "Processing (auth)".to_string(),
        InputNoteState::ProcessingUnauthenticated(_) => "Processing (unauth)".to_string(),
        InputNoteState::ConsumedAuthenticatedLocal(_) => "Consumed (auth local)".to_string(),
        InputNoteState::ConsumedUnauthenticatedLocal(_) => "Consumed (unauth local)".to_string(),
        InputNoteState::ConsumedExternal(_) => "Consumed (external)".to_string(),
    }
}

fn format_output_note_state(note: &miden_client::store::OutputNoteRecord) -> String {
    use miden_client::store::OutputNoteState;
    match note.state() {
        OutputNoteState::ExpectedPartial => "Expected (partial)".to_string(),
        OutputNoteState::ExpectedFull { .. } => "Expected (full)".to_string(),
        OutputNoteState::CommittedPartial { .. } => "Committed (partial)".to_string(),
        OutputNoteState::CommittedFull { .. } => "Committed (full)".to_string(),
        OutputNoteState::Consumed { .. } => "Consumed".to_string(),
    }
}

fn count_notes_per_block<T>(notes: &[T]) -> HashMap<u32, usize>
where
    T: NoteWithInclusion,
{
    let mut counts = HashMap::new();
    for note in notes {
        if let Some(proof) = note.inclusion_proof() {
            let block_num = proof.location().block_num().as_u32();
            *counts.entry(block_num).or_insert(0) += 1;
        }
    }
    counts
}

fn query_u64(conn: &Connection, sql: &str) -> Result<u64> {
    let value: i64 = conn.query_row(sql, [], |row| row.get(0))?;
    Ok(value.try_into().unwrap_or(0))
}

trait NoteWithInclusion {
    fn inclusion_proof(&self) -> Option<&miden_client::note::NoteInclusionProof>;
}

impl NoteWithInclusion for miden_client::store::InputNoteRecord {
    fn inclusion_proof(&self) -> Option<&miden_client::note::NoteInclusionProof> {
        self.inclusion_proof()
    }
}

impl NoteWithInclusion for miden_client::store::OutputNoteRecord {
    fn inclusion_proof(&self) -> Option<&miden_client::note::NoteInclusionProof> {
        self.inclusion_proof()
    }
}
