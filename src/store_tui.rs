use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
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
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Tabs},
};
use tokio::runtime::{Handle, Runtime};

use crate::render::note::{format_note_tag, well_known_label_from_root};
use rusqlite::{Connection, params};

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
}

pub(crate) fn run_store_tui(store_path: PathBuf) -> Result<()> {
    let rt = Runtime::new()?;
    let _guard = rt.enter();
    let handle = rt.handle().clone();
    let store = handle
        .block_on(SqliteStore::new(store_path.clone()))
        .with_context(|| format!("failed to open store at {}", store_path.display()))?;

    let mut app = StoreTui::new(store_path, store, handle)?;
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

struct StoreTui {
    store_path: PathBuf,
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
    filters: Vec<String>,
    visible: Vec<Vec<usize>>,
    filter_mode: bool,
    filter_input: String,
    status: String,
}

impl StoreTui {
    fn new(store_path: PathBuf, store: SqliteStore, handle: Handle) -> Result<Self> {
        let conn = Connection::open(&store_path)
            .with_context(|| format!("failed to open store at {}", store_path.display()))?;
        Ok(Self {
            store_path,
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

        self.input_notes = self.handle.block_on(self.store.get_input_notes(NoteFilter::All))?;
        self.output_notes_total = query_u64(&self.conn, "SELECT COUNT(*) FROM output_notes")?;
        self.output_notes.clear();
        self.output_notes_loaded = false;
        self.transactions = self
            .handle
            .block_on(self.store.get_transactions(TransactionFilter::All))?;
        self.block_headers = self.handle.block_on(self.store.get_tracked_block_headers())?;
        self.block_headers.sort_by_key(|header| header.block_num());

        self.mmr_nodes = self
            .handle
            .block_on(self.store.get_partial_blockchain_nodes(PartialBlockchainFilter::All))?;
        self.mmr_peaks = self
            .block_headers
            .last()
            .and_then(|header| {
                self.handle
                    .block_on(
                        self.store
                            .get_partial_blockchain_peaks_by_block_num(header.block_num()),
                    )
                    .ok()
            });

        self.input_notes_per_block = count_notes_per_block(&self.input_notes);
        self.output_notes_per_block.clear();

        self.status = format!(
            "store: {} | accounts: {} | input notes: {} | output notes: {}/{} | txs: {} | blocks: {}",
            self.store_path.display(),
            self.accounts.len(),
            self.input_notes.len(),
            self.output_notes.len(),
            self.output_notes_total,
            self.transactions.len(),
            self.block_headers.len()
        );
        self.rebuild_visible_all();

        Ok(())
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
            KeyCode::Left => self.prev_tab(),
            KeyCode::Right => self.next_tab(),
            KeyCode::Tab => self.next_tab(),
            KeyCode::Up => self.move_selection(-1),
            KeyCode::Down => self.move_selection(1),
            KeyCode::Char('/') => {
                self.filter_mode = true;
                self.filter_input = self.filters[self.tab].clone();
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
    }

    fn next_tab(&mut self) {
        self.tab = (self.tab + 1) % Tab::all().len();
        if self.current_tab() == Tab::OutputNotes {
            if let Err(err) = self.ensure_output_notes_loaded() {
                self.status = format!("output notes load failed: {err}");
            }
        }
        self.rebuild_visible(self.tab);
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
            .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
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

        let details = Paragraph::new(self.current_detail_lines())
            .block(Block::default().borders(Borders::ALL).title("Details"));
        frame.render_widget(details, body[1]);

        let footer = Paragraph::new(Line::from(self.status_line()))
            .style(Style::default().fg(Color::White))
            .block(Block::default().borders(Borders::ALL).title("Status"));
        frame.render_widget(footer, layout[2]);
    }

    fn status_line(&self) -> String {
        let hint = "keys: / filter  r refresh  q quit  arrows move  tab switch";
        let base = if self.status.is_empty() {
            hint.to_string()
        } else {
            format!("{} | {}", self.status, hint)
        };

        if self.filter_mode {
            format!(
                "{} | filter ({}): {}",
                base,
                self.current_tab().title(),
                self.filter_input
            )
        } else if !self.filters[self.tab].is_empty() {
            format!(
                "{} | filter ({}): {}",
                base,
                self.current_tab().title(),
                self.filters[self.tab]
            )
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
                    ListItem::new(note.id().to_string())
                })
                .collect(),
            Tab::OutputNotes => {
                if !self.output_notes_loaded {
                    return vec![ListItem::new("output notes not loaded")];
                }
                indices
                    .into_iter()
                    .map(|idx| {
                        let note = &self.output_notes[idx];
                        ListItem::new(note.id().to_string())
                    })
                    .collect()
            }
            Tab::Transactions => indices
                .into_iter()
                .map(|idx| {
                    let tx = &self.transactions[idx];
                    ListItem::new(format!("{} ({})", tx.id, tx.status))
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
            return vec![Line::from("output notes not loaded")];
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

    fn account_detail(&self, idx: usize) -> Vec<Line<'static>> {
        let id = self.accounts[idx];
        let mut lines = vec![Line::from(format!("account id: {id}"))];
        if let Some(header) = self
            .account_headers
            .get(idx)
            .and_then(|h| h.clone())
        {
            lines.push(Line::from(format!("nonce: {}", header.nonce())));
            lines.push(Line::from(format!("vault: {}", header.vault_root())));
            lines.push(Line::from(format!("storage: {}", header.storage_commitment())));
            lines.push(Line::from(format!("code: {}", header.code_commitment())));
        } else {
            lines.push(Line::from("header: n/a"));
        }

        match self.account_history(id) {
            Ok(history) => {
                lines.push(Line::from(format!("states: {}", history.total)));
                if !history.rows.is_empty() {
                    lines.push(Line::from("recent states (nonce, commitment):"));
                    for row in history.rows {
                        lines.push(Line::from(format!(
                            "- {} | {}",
                            row.nonce, row.account_commitment
                        )));
                    }
                }
            }
            Err(err) => {
                lines.push(Line::from(format!("history: error ({err})")));
            }
        }
        lines
    }

    fn input_note_detail(&self, idx: usize) -> Vec<Line<'static>> {
        let note = &self.input_notes[idx];
        let details = note.details();
        let script_root = details.script().root();
        let script_label = match well_known_label_from_root(&script_root) {
            Some(label) => format!("{script_root} ({label})"),
            None => script_root.to_string(),
        };

        let mut lines = vec![
            Line::from(format!("id: {}", note.id())),
            Line::from(format!("state: {:?}", note.state())),
            Line::from(format!("script root: {script_label}")),
        ];
        if let Some(commitment) = note.commitment() {
            lines.push(Line::from(format!("commitment: {commitment}")));
        }

        if let Some(metadata) = note.metadata() {
            lines.push(Line::from(format!("sender: {}", metadata.sender())));
            lines.push(Line::from(format!("type: {:?}", metadata.note_type())));
            lines.push(Line::from(format!("tag: {}", format_note_tag(metadata.tag()))));
        }

        lines.push(Line::from(format!(
            "assets: {}",
            details.assets().num_assets()
        )));
        lines.push(Line::from(format!("inputs: {}", details.inputs().values().len())));
        lines
    }

    fn output_note_detail(&self, idx: usize) -> Vec<Line<'static>> {
        let note = &self.output_notes[idx];
        let mut lines = vec![
            Line::from(format!("id: {}", note.id())),
            Line::from(format!("state: {:?}", note.state())),
            Line::from(format!("expected height: {}", note.expected_height().as_u32())),
            Line::from(format!("sender: {}", note.metadata().sender())),
            Line::from(format!("type: {:?}", note.metadata().note_type())),
            Line::from(format!("tag: {}", format_note_tag(note.metadata().tag()))),
            Line::from(format!("assets: {}", note.assets().num_assets())),
        ];
        let commitment = NoteHeader::new(note.id(), note.metadata().clone()).commitment();
        lines.push(Line::from(format!("commitment: {commitment}")));

        if let Some(recipient) = note.recipient() {
            let script_root = recipient.script().root();
            let script_label = match well_known_label_from_root(&script_root) {
                Some(label) => format!("{script_root} ({label})"),
                None => script_root.to_string(),
            };
            lines.push(Line::from(format!("script root: {script_label}")));
            lines.push(Line::from(format!(
                "inputs: {}",
                recipient.inputs().values().len()
            )));
        } else {
            lines.push(Line::from("recipient: n/a"));
        }

        lines
    }

    fn block_detail(&self, idx: usize) -> Vec<Line<'static>> {
        let header = &self.block_headers[idx];
        let block_num = header.block_num().as_u32();
        let mut lines = vec![
            Line::from(format!("block: {}", block_num)),
            Line::from(format!("chain commitment: {}", header.chain_commitment())),
            Line::from(format!("account root: {}", header.account_root())),
            Line::from(format!("nullifier root: {}", header.nullifier_root())),
            Line::from(format!("note root: {}", header.note_root())),
            Line::from(format!("tx commitment: {}", header.tx_commitment())),
            Line::from(format!("timestamp: {}", header.timestamp())),
        ];

        let input_count = self.input_notes_per_block.get(&block_num).copied().unwrap_or(0);
        let output_count = self.output_notes_per_block.get(&block_num).copied();
        lines.push(Line::from(format!(
            "notes in block: input {} | output {}",
            input_count,
            output_count
                .map(|count| count.to_string())
                .unwrap_or_else(|| "n/a (load output notes)".to_string())
        )));

        lines.push(Line::from(format!(
            "mmr nodes tracked: {}",
            self.mmr_nodes.len()
        )));
        if let Some(peaks) = &self.mmr_peaks {
            lines.push(Line::from(format!(
                "mmr peaks: {} | leaves: {}",
                peaks.num_peaks(),
                peaks.num_leaves()
            )));
            for (idx, peak) in peaks.peaks().iter().enumerate().take(6) {
                lines.push(Line::from(format!("peak[{idx}]: {peak}")));
            }
        } else {
            lines.push(Line::from("mmr peaks: n/a"));
        }

        lines
    }

    fn transaction_detail(&self, idx: usize) -> Vec<Line<'static>> {
        let tx = &self.transactions[idx];
        let details = &tx.details;
        vec![
            Line::from(format!("id: {}", tx.id)),
            Line::from(format!("status: {}", tx.status)),
            Line::from(format!("account: {}", details.account_id)),
            Line::from(format!("block: {}", details.block_num.as_u32())),
            Line::from(format!(
                "submission height: {}",
                details.submission_height.as_u32()
            )),
            Line::from(format!(
                "expiration block: {}",
                details.expiration_block_num.as_u32()
            )),
            Line::from(format!(
                "input nullifiers: {}",
                details.input_note_nullifiers.len()
            )),
            Line::from(format!(
                "output notes: {}",
                details.output_notes.num_notes()
            )),
        ]
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
        self.status = format!(
            "store: {} | accounts: {} | input notes: {} | output notes: {}/{} | txs: {} | blocks: {}",
            self.store_path.display(),
            self.accounts.len(),
            self.input_notes.len(),
            self.output_notes.len(),
            self.output_notes_total,
            self.transactions.len(),
            self.block_headers.len()
        );
        self.rebuild_visible(self.tab);
        Ok(())
    }

    fn account_history(&self, account_id: miden_client::account::AccountId) -> Result<AccountHistory> {
        let id_value: u128 = account_id.into();
        let id_value: i64 = id_value
            .try_into()
            .map_err(|_| anyhow!("account id out of range for sqlite storage"))?;

        let total: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM accounts WHERE id = ?",
                params![id_value],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let mut stmt = self.conn.prepare(
            "SELECT nonce, account_commitment FROM accounts WHERE id = ? ORDER BY nonce DESC LIMIT 5",
        )?;
        let rows = stmt.query_map(params![id_value], |row| {
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

struct AccountHistoryRow {
    nonce: i64,
    account_commitment: String,
}

struct AccountHistory {
    total: u64,
    rows: Vec<AccountHistoryRow>,
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
