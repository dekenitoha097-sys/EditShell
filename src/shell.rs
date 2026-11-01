use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, RawFd};
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug)]
pub enum SessionEvent {
    Output(String),
    ClearOutput,
    TerminalScreenStarted,
    TerminalOutput(Vec<u8>),
    TerminalScreenEnded,
    Finished { status: i32, cwd: PathBuf },
    Failed(String),
    Exited,
}

pub struct ShellSession {
    commands: Option<SyncSender<String>>,
    events: Receiver<SessionEvent>,
    running: Arc<AtomicBool>,
    child: Arc<Mutex<Child>>,
    terminal_input: Arc<Mutex<File>>,
    process_group: i32,
}

impl ShellSession {
    pub fn start(cwd: Option<PathBuf>) -> io::Result<Self> {
        let cwd = cwd.unwrap_or(std::env::current_dir()?);
        let mut master_fd: RawFd = -1;
        let mut slave_fd: RawFd = -1;
        let window_size = libc::winsize {
            ws_row: 40,
            ws_col: 120,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        let open_result = unsafe {
            libc::openpty(
                &mut master_fd,
                &mut slave_fd,
                std::ptr::null_mut(),
                std::ptr::null(),
                &window_size,
            )
        };
        if open_result == -1 {
            return Err(io::Error::last_os_error());
        }

        let master = unsafe { File::from_raw_fd(master_fd) };
        let slave = unsafe { File::from_raw_fd(slave_fd) };
        let mut terminal_settings = unsafe { std::mem::zeroed::<libc::termios>() };
        if unsafe { libc::tcgetattr(slave.as_raw_fd(), &mut terminal_settings) } == -1 {
            return Err(io::Error::last_os_error());
        }
        terminal_settings.c_lflag &= !(libc::ECHO | libc::ECHONL);
        if unsafe { libc::tcsetattr(slave.as_raw_fd(), libc::TCSANOW, &terminal_settings) } == -1 {
            return Err(io::Error::last_os_error());
        }
        let terminal_input = Arc::new(Mutex::new(master.try_clone()?));
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
            .to_string();
        let mut command = Command::new("bash");
        command
            .args(["--noprofile", "--norc", "--noediting", "-i"])
            .current_dir(&cwd)
            .env("TERM", "xterm-256color")
            .env("PS1", "")
            .env("HISTFILE", "/dev/null")
            .env(
                "PROMPT_COMMAND",
                format!(
                    "__editshell_status=$?; alias ls='ls --color=always'; __editshell_pwd=$(printf '%s' \"$PWD\" | base64 -w0); printf '\\036EDITSHELL:{nonce}:%s:%s\\037\\n' \"$__editshell_status\" \"$__editshell_pwd\""
                ),
            )
            .stdin(Stdio::from(slave.try_clone()?))
            .stdout(Stdio::from(slave.try_clone()?))
            .stderr(Stdio::from(slave.try_clone()?));
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() == -1 {
                    return Err(io::Error::last_os_error());
                }
                if libc::ioctl(0, libc::TIOCSCTTY, 0) == -1 {
                    return Err(io::Error::last_os_error());
                }
                if libc::tcsetpgrp(0, libc::getpgrp()) == -1 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let child = command.spawn()?;
        let process_group = child.id() as i32;
        drop(slave);
        let (command_tx, command_rx) = mpsc::sync_channel::<String>(1);
        let (event_tx, event_rx) = mpsc::channel();
        let running = Arc::new(AtomicBool::new(false));
        let child = Arc::new(Mutex::new(child));

        let writer_events = event_tx.clone();
        let writer_terminal = Arc::clone(&terminal_input);
        thread::spawn(move || {
            for command in command_rx {
                let result = writer_terminal
                    .lock()
                    .map_err(|_| io::Error::other("terminal writer lock poisoned"))
                    .and_then(|mut terminal| {
                        terminal.write_all(command.as_bytes())?;
                        terminal.write_all(b"\n")?;
                        terminal.flush()
                    });
                if let Err(error) = result {
                    let _ = writer_events.send(SessionEvent::Failed(format!(
                        "Écriture vers Bash impossible : {error}"
                    )));
                    break;
                }
            }
        });

        let reader_events = event_tx.clone();
        let reader_running = Arc::clone(&running);
        let initial_prompt = Arc::new(AtomicBool::new(true));
        let reader_initial_prompt = Arc::clone(&initial_prompt);
        let mut decoder = PtyOutputParser::new(nonce.clone());
        let reader = master;
        thread::spawn(move || {
            let mut reader = reader;
            let mut bytes = [0; 4096];
            loop {
                match reader.read(&mut bytes) {
                    Ok(0) => {
                        for event in decoder.finish() {
                            let _ = reader_events.send(event);
                        }
                        reader_running.store(false, Ordering::Release);
                        let _ = reader_events.send(SessionEvent::Exited);
                        break;
                    }
                    Ok(count) => {
                        for event in decoder.push(&bytes[..count]) {
                            if matches!(event, SessionEvent::Finished { .. }) {
                                if reader_initial_prompt.swap(false, Ordering::AcqRel) {
                                    continue;
                                }
                                if reader_running.swap(false, Ordering::AcqRel) {
                                    let _ = reader_events.send(event);
                                }
                            } else {
                                let _ = reader_events.send(event);
                            }
                        }
                    }
                    Err(error) if error.raw_os_error() == Some(libc::EIO) => {
                        for event in decoder.finish() {
                            let _ = reader_events.send(event);
                        }
                        reader_running.store(false, Ordering::Release);
                        let _ = reader_events.send(SessionEvent::Exited);
                        break;
                    }
                    Err(error) => {
                        reader_running.store(false, Ordering::Release);
                        let _ = reader_events.send(SessionEvent::Failed(format!(
                            "Lecture de la sortie de Bash impossible : {error}"
                        )));
                        break;
                    }
                }
            }
        });

        Ok(Self {
            commands: Some(command_tx),
            events: event_rx,
            running,
            child,
            terminal_input,
            process_group,
        })
    }

    pub fn execute(&self, command: &str) -> Result<(), &'static str> {
        if command.trim().is_empty() {
            return Err("La commande est vide.");
        }
        self.running
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| "Une commande est déjà en cours.")?;

        let wrapper = format!("eval {}", bash_quote(command));
        let Some(sender) = &self.commands else {
            self.running.store(false, Ordering::Release);
            return Err("La session Bash est fermée.");
        };
        if sender.send(wrapper).is_err() {
            self.running.store(false, Ordering::Release);
            return Err("Impossible d’envoyer la commande à Bash.");
        }
        Ok(())
    }

    pub fn try_event(&self) -> Option<SessionEvent> {
        self.events.try_recv().ok()
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }

    pub fn interrupt(&self) -> io::Result<()> {
        if !self.is_running() {
            return Ok(());
        }
        let terminal = self
            .terminal_input
            .lock()
            .map_err(|_| io::Error::other("terminal writer lock poisoned"))?;
        let foreground_group = unsafe { libc::tcgetpgrp(terminal.as_raw_fd()) };
        if foreground_group == -1 {
            return Err(io::Error::last_os_error());
        }
        let result = unsafe { libc::kill(-foreground_group, libc::SIGINT) };
        if result == -1 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    pub fn send_input(&self, input: &str) -> io::Result<()> {
        if !self.is_running() {
            return Err(io::Error::other(
                "aucune commande interactive n’est en cours",
            ));
        }
        let mut terminal = self
            .terminal_input
            .lock()
            .map_err(|_| io::Error::other("terminal writer lock poisoned"))?;
        terminal.write_all(input.as_bytes())?;
        terminal.write_all(b"\n")?;
        terminal.flush()
    }

    pub fn send_raw_input(&self, input: &[u8]) -> io::Result<()> {
        if !self.is_running() {
            return Err(io::Error::other(
                "aucune commande interactive n’est en cours",
            ));
        }
        let mut terminal = self
            .terminal_input
            .lock()
            .map_err(|_| io::Error::other("terminal writer lock poisoned"))?;
        terminal.write_all(input)?;
        terminal.flush()
    }

    pub fn resize_terminal(&self, columns: u16, rows: u16) -> io::Result<()> {
        let terminal = self
            .terminal_input
            .lock()
            .map_err(|_| io::Error::other("terminal writer lock poisoned"))?;
        let size = libc::winsize {
            ws_row: rows,
            ws_col: columns,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        if unsafe { libc::ioctl(terminal.as_raw_fd(), libc::TIOCSWINSZ, &size) } == -1 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}

impl Drop for ShellSession {
    fn drop(&mut self) {
        self.commands.take();
        if let Ok(mut child) = self.child.lock()
            && child.try_wait().ok().flatten().is_none()
        {
            unsafe {
                libc::kill(-self.process_group, libc::SIGHUP);
            }
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

pub fn bash_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\"'\"'"))
}

struct PtyOutputParser {
    pending: Vec<u8>,
    nonce: String,
    skip_marker_newline: bool,
    in_alternate_screen: bool,
}

impl PtyOutputParser {
    fn new(nonce: String) -> Self {
        Self {
            pending: Vec::new(),
            nonce,
            skip_marker_newline: false,
            in_alternate_screen: false,
        }
    }

    fn push(&mut self, bytes: &[u8]) -> Vec<SessionEvent> {
        self.pending.extend_from_slice(bytes);
        let mut events = Vec::new();
        loop {
            if self.skip_marker_newline {
                if self.pending.starts_with(b"\r\n") {
                    self.pending.drain(..2);
                    self.skip_marker_newline = false;
                } else if self.pending.starts_with(b"\n") {
                    self.pending.drain(..1);
                    self.skip_marker_newline = false;
                } else if self.pending.starts_with(b"\r") && self.pending.len() == 1 {
                    break;
                } else {
                    self.skip_marker_newline = false;
                }
            }

            if let Some(marker_start) = self.pending.iter().position(|byte| *byte == 0x1e) {
                if marker_start > 0 {
                    let output = self.pending[..marker_start].to_vec();
                    self.emit_output(&output, &mut events);
                }
                let Some(marker_end) = self.pending[marker_start + 1..]
                    .iter()
                    .position(|byte| *byte == 0x1f)
                    .map(|offset| marker_start + 1 + offset)
                else {
                    self.pending.drain(..marker_start);
                    break;
                };
                let marker = String::from_utf8_lossy(&self.pending[marker_start + 1..marker_end]);
                if let Some((status, cwd)) = parse_marker_payload(&marker, &self.nonce) {
                    events.push(SessionEvent::Finished { status, cwd });
                    self.pending.drain(..marker_end + 1);
                    self.skip_marker_newline = true;
                } else {
                    let output = self.pending[..marker_end + 1].to_vec();
                    self.emit_output(&output, &mut events);
                    self.pending.drain(..marker_end + 1);
                }
                continue;
            }

            let mut safe_len = self.pending.len();
            if self.pending.last() == Some(&0x1e) {
                safe_len -= 1;
            }
            if let Some(escape_start) = self.pending[..safe_len]
                .iter()
                .rposition(|byte| *byte == 0x1b)
                && !terminal_escape_is_complete(&self.pending[escape_start..safe_len])
            {
                safe_len = escape_start;
            }
            if safe_len == 0 {
                break;
            }
            let output = self.pending[..safe_len].to_vec();
            self.emit_output(&output, &mut events);
            self.pending.drain(..safe_len);
        }
        events
    }

    fn finish(&mut self) -> Vec<SessionEvent> {
        let mut events = Vec::new();
        if !self.pending.is_empty() {
            let pending = std::mem::take(&mut self.pending);
            self.emit_output(&pending, &mut events);
        }
        if self.in_alternate_screen {
            self.in_alternate_screen = false;
            events.push(SessionEvent::TerminalScreenEnded);
        }
        events
    }

    fn emit_output(&mut self, bytes: &[u8], events: &mut Vec<SessionEvent>) {
        let mut cursor = 0;
        let mut scan = 0;
        while scan + 1 < bytes.len() {
            if bytes[scan] == 0x1b && bytes[scan + 1] == b'[' {
                if let Some(relative_end) = bytes[scan + 2..]
                    .iter()
                    .position(|byte| (0x40..=0x7e).contains(byte))
                {
                    let end = scan + 2 + relative_end;
                    match alternate_screen_mode(&bytes[scan..=end]) {
                        Some(true) if !self.in_alternate_screen => {
                            self.emit_plain_output(&bytes[cursor..scan], events);
                            self.in_alternate_screen = true;
                            events.push(SessionEvent::TerminalScreenStarted);
                            cursor = scan;
                        }
                        Some(false) if self.in_alternate_screen => {
                            events.push(SessionEvent::TerminalOutput(bytes[cursor..=end].to_vec()));
                            self.in_alternate_screen = false;
                            events.push(SessionEvent::TerminalScreenEnded);
                            cursor = end + 1;
                        }
                        _ => {}
                    }
                    scan = end + 1;
                    continue;
                }
            }
            scan += 1;
        }

        if cursor < bytes.len() {
            if self.in_alternate_screen {
                events.push(SessionEvent::TerminalOutput(bytes[cursor..].to_vec()));
            } else {
                self.emit_plain_output(&bytes[cursor..], events);
            }
        }
    }

    fn emit_plain_output(&self, bytes: &[u8], events: &mut Vec<SessionEvent>) {
        if bytes.is_empty() {
            return;
        }
        let text = String::from_utf8_lossy(bytes);
        let (output, cleared) = strip_terminal_sequences(&text);
        if cleared {
            events.push(SessionEvent::ClearOutput);
        }
        if !output.is_empty() {
            events.push(SessionEvent::Output(output));
        }
    }
}

fn alternate_screen_mode(sequence: &[u8]) -> Option<bool> {
    let final_byte = *sequence.last()?;
    let enabled = match final_byte {
        b'h' => true,
        b'l' => false,
        _ => return None,
    };
    let parameters = std::str::from_utf8(sequence.get(2..sequence.len() - 1)?).ok()?;
    parameters
        .split(';')
        .any(|parameter| matches!(parameter, "?47" | "?1047" | "?1049"))
        .then_some(enabled)
}

fn terminal_escape_is_complete(bytes: &[u8]) -> bool {
    if bytes.len() < 2 || bytes[0] != 0x1b {
        return true;
    }
    match bytes[1] {
        b'[' => bytes[2..].iter().any(|byte| (0x40..=0x7e).contains(byte)),
        b']' => bytes[2..].contains(&0x07) || bytes.windows(2).any(|pair| pair == b"\x1b\\"),
        _ => true,
    }
}

fn parse_marker_payload(marker: &str, nonce: &str) -> Option<(i32, PathBuf)> {
    let payload = marker.strip_prefix(&format!("EDITSHELL:{nonce}:"))?;
    let (status, cwd) = payload.split_once(':')?;
    let status = status.parse().ok()?;
    let cwd = STANDARD.decode(cwd).ok()?;
    let cwd = String::from_utf8(cwd).ok()?;
    Some((status, PathBuf::from(cwd)))
}

fn strip_terminal_sequences(text: &str) -> (String, bool) {
    let mut output = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut cleared = false;
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            match chars.next() {
                Some('[') => {
                    let mut sequence = String::new();
                    for code in chars.by_ref() {
                        if ('\u{40}'..='\u{7e}').contains(&code) {
                            if code == 'J'
                                && sequence
                                    .split(';')
                                    .any(|param| param == "2" || param == "3")
                            {
                                cleared = true;
                            }
                            if code == 'm' {
                                output.push_str("\u{1b}[");
                                output.push_str(&sequence);
                                output.push('m');
                            }
                            break;
                        }
                        sequence.push(code);
                    }
                }
                Some(']') => {
                    while let Some(code) = chars.next() {
                        if code == '\u{7}' {
                            break;
                        }
                        if code == '\u{1b}' && chars.peek() == Some(&'\\') {
                            chars.next();
                            break;
                        }
                    }
                }
                Some(_) | None => {}
            }
        } else if ch == '\t' || ch == '\n' || !ch.is_control() {
            output.push(ch);
        }
    }
    (output, cleared)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn bash_quoting_preserves_arbitrary_text() {
        assert_eq!(bash_quote("it's fine"), "'it'\"'\"'s fine'");
        assert_eq!(bash_quote("a\nb"), "'a\nb'");
    }

    #[test]
    fn marker_round_trips_status_and_directory() {
        let cwd = STANDARD.encode("/tmp/shell test");
        let line = format!("\u{1e}EDITSHELL:abc:7:{cwd}\u{1f}\n");
        let mut decoder = PtyOutputParser::new("abc".to_owned());
        assert!(decoder.push(line.as_bytes()).iter().any(|event| matches!(
            event,
            SessionEvent::Finished { status: 7, cwd } if cwd == &PathBuf::from("/tmp/shell test")
        )));
        let mut decoder = PtyOutputParser::new("other".to_owned());
        assert!(
            !decoder
                .push(line.as_bytes())
                .iter()
                .any(|event| matches!(event, SessionEvent::Finished { .. }))
        );
    }

    #[test]
    fn terminal_clear_sequences_are_consumed_and_marker_is_found() {
        let cwd = STANDARD.encode("/tmp");
        let line = format!("\u{1b}[H\u{1b}[2J\u{1b}[3J\u{1e}EDITSHELL:abc:0:{cwd}\u{1f}\n");
        let mut decoder = PtyOutputParser::new("abc".to_owned());
        let events = decoder.push(line.as_bytes());
        assert!(
            events
                .iter()
                .any(|event| matches!(event, SessionEvent::ClearOutput))
        );
        assert!(events.iter().any(|event| matches!(
            event,
            SessionEvent::Finished { status: 0, cwd } if cwd == &PathBuf::from("/tmp")
        )));
        assert!(
            !events.iter().any(
                |event| matches!(event, SessionEvent::Output(text) if text.contains('\u{1b}'))
            )
        );
    }

    #[test]
    fn alternate_screen_is_routed_to_interactive_terminal() {
        let mut decoder = PtyOutputParser::new("nonce".to_owned());
        let events = decoder.push(b"before\x1b[?1049h\x1b[2Jchoice\x1b[?1049lafter");
        assert!(matches!(events[0], SessionEvent::Output(ref text) if text == "before"));
        assert!(matches!(events[1], SessionEvent::TerminalScreenStarted));
        assert!(
            matches!(events[2], SessionEvent::TerminalOutput(ref bytes) if bytes == b"\x1b[?1049h\x1b[2Jchoice\x1b[?1049l")
        );
        assert!(matches!(events[3], SessionEvent::TerminalScreenEnded));
        assert!(matches!(events[4], SessionEvent::Output(ref text) if text == "after"));
    }

    #[test]
    fn alternate_screen_detection_handles_split_escape_sequences() {
        let mut decoder = PtyOutputParser::new("nonce".to_owned());
        assert!(decoder.push(b"\x1b[?104").is_empty());
        let events = decoder.push(b"9hprompt");
        assert!(matches!(events[0], SessionEvent::TerminalScreenStarted));
        assert!(
            matches!(events[1], SessionEvent::TerminalOutput(ref bytes) if bytes == b"\x1b[?1049hprompt")
        );
        assert!(matches!(
            decoder.push(b"\x1b[?1049l").last(),
            Some(SessionEvent::TerminalScreenEnded)
        ));
    }

    #[test]
    fn ansi_color_sequences_are_preserved_for_styled_output() {
        let (output, cleared) = strip_terminal_sequences("\u{1b}[1;31mred\u{1b}[0m");
        assert_eq!(output, "\u{1b}[1;31mred\u{1b}[0m");
        assert!(!cleared);
    }

    #[test]
    fn ls_uses_color_output_by_default() {
        let mut session = ShellSession::start(None).expect("start Bash");
        session
            .execute("ls --color=always")
            .expect("execute colorized ls");
        let output = wait_for_result(&mut session);
        assert!(output.contains("\u{1b}["), "{output:?}");
    }

    #[test]
    fn unterminated_interactive_prompts_are_emitted_immediately() {
        let mut decoder = PtyOutputParser::new("nonce".to_owned());
        let events = decoder.push(b"Mot de passe : ");
        assert!(
            events.iter().any(
                |event| matches!(event, SessionEvent::Output(text) if text == "Mot de passe : ")
            )
        );
    }

    #[test]
    fn clear_command_emits_a_clear_event_and_keeps_shell_alive() {
        let session = ShellSession::start(None).expect("start Bash");
        session.execute("clear").expect("execute clear");
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut cleared = false;
        let mut finished = false;
        let mut raw_escape = false;
        while Instant::now() < deadline {
            if let Some(event) = session.try_event() {
                match event {
                    SessionEvent::ClearOutput => cleared = true,
                    SessionEvent::Output(text) => raw_escape |= text.contains('\u{1b}'),
                    SessionEvent::TerminalOutput(bytes) => {
                        raw_escape |= bytes.contains(&0x1b);
                    }
                    SessionEvent::TerminalScreenStarted | SessionEvent::TerminalScreenEnded => {}
                    SessionEvent::Finished { .. } => {
                        finished = true;
                        break;
                    }
                    SessionEvent::Failed(error) => panic!("{error}"),
                    SessionEvent::Exited => panic!("shell exited unexpectedly"),
                }
            }
            thread::sleep(Duration::from_millis(10));
        }
        assert!(finished, "clear should finish normally");
        assert!(cleared, "clear must clear the visible output");
        assert!(!raw_escape, "terminal escapes should not be shown raw");
    }

    #[test]
    fn ctrl_c_interrupts_command_and_keeps_session_usable() {
        let mut session = ShellSession::start(None).expect("start Bash");
        session.execute("sleep 30").expect("execute sleep");
        thread::sleep(Duration::from_millis(500));
        session.interrupt().expect("send SIGINT");
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut status = None;
        while Instant::now() < deadline {
            if let Some(SessionEvent::Finished { status: result, .. }) = session.try_event() {
                status = Some(result);
                break;
            }

            thread::sleep(Duration::from_millis(10));
        }
        assert!(
            status.is_some(),
            "interrupted command should finish promptly"
        );

        session.execute("printf 'session-alive\\n'").unwrap();
        assert!(wait_for_result(&mut session).contains("session-alive"));
    }

    #[test]
    fn exit_command_emits_session_ended_event() {
        let session = ShellSession::start(None).expect("start Bash");
        session.execute("exit").expect("execute exit");
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if let Some(event) = session.try_event() {
                match event {
                    SessionEvent::Exited => {
                        assert!(!session.is_running());
                        return;
                    }
                    SessionEvent::Failed(error) => panic!("{error}"),
                    _ => {}
                }
            }
            thread::sleep(Duration::from_millis(10));
        }
        panic!("exit should end the shell session");
    }

    #[test]
    fn interactive_input_reaches_running_command() {
        let mut session = ShellSession::start(None).expect("start Bash");
        session
            .execute("read -r answer; printf 'received:%s\\n' \"$answer\"")
            .expect("execute read");
        thread::sleep(Duration::from_millis(100));
        session
            .send_input("yes")
            .expect("send interactive response");
        assert!(wait_for_result(&mut session).contains("received:yes"));
    }

    #[test]
    fn raw_terminal_input_reaches_running_command_without_newline() {
        let mut session = ShellSession::start(None).expect("start Bash");
        session
            .execute("IFS= read -r -n 1 answer; printf 'received:%s\\n' \"$answer\"")
            .expect("execute single-character read");
        thread::sleep(Duration::from_millis(100));
        session
            .send_raw_input(b"x")
            .expect("send terminal key without newline");
        assert!(wait_for_result(&mut session).contains("received:x"));
    }

    #[test]
    fn hidden_password_prompt_accepts_input_without_echoing_it() {
        let mut session = ShellSession::start(None).expect("start Bash");
        session
            .execute("read -r -s -p 'Mot de passe : ' password; printf '\\naccepted\\n'")
            .expect("execute hidden prompt");
        thread::sleep(Duration::from_millis(100));
        session
            .send_input("test-secret")
            .expect("send password response");
        let output = wait_for_result(&mut session);
        assert!(output.contains("Mot de passe : "), "{output:?}");
        assert!(output.contains("accepted"), "{output:?}");
        assert!(
            !output.contains("test-secret"),
            "password leaked: {output:?}"
        );
    }

    #[test]
    fn commands_share_directory_and_environment() {
        let mut session = ShellSession::start(None).expect("start Bash");
        run_command(&mut session, "cd /tmp && export EDITSHELL_TEST=value");
        session
            .execute("printf '%s\\n' \"$PWD:$EDITSHELL_TEST\"")
            .unwrap();
        let output = wait_for_result(&mut session);
        assert!(output.contains("/tmp:value"), "{output:?}");
    }

    #[test]
    fn multiline_pipelines_and_failure_status_work() {
        let mut session = ShellSession::start(None).expect("start Bash");
        session
            .execute("printf 'alpha\\nbeta\\n' | grep beta")
            .unwrap();
        assert!(wait_for_result(&mut session).contains("beta"));

        session.execute("false").unwrap();
        let mut status = None;
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if let Some(SessionEvent::Finished { status: result, .. }) = session.try_event() {
                status = Some(result);
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(status, Some(1));

        session
            .execute("for item in a b; do\n  printf '%s\\n' \"$item\"\ndone")
            .unwrap();
        let output = wait_for_result(&mut session);
        assert!(output.contains("a\nb\n"), "{output:?}");
    }

    fn run_command(session: &mut ShellSession, command: &str) {
        session.execute(command).expect("execute");
        let _ = wait_for_result(session);
    }

    fn wait_for_result(session: &mut ShellSession) -> String {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut output = String::new();
        while Instant::now() < deadline {
            while let Some(event) = session.try_event() {
                match event {
                    SessionEvent::Output(text) => output.push_str(&text),
                    SessionEvent::TerminalOutput(bytes) => {
                        output.push_str(&String::from_utf8_lossy(&bytes));
                    }
                    SessionEvent::ClearOutput => output.clear(),
                    SessionEvent::TerminalScreenStarted | SessionEvent::TerminalScreenEnded => {}
                    SessionEvent::Finished { .. } => return output,
                    SessionEvent::Failed(error) => panic!("{error}"),
                    SessionEvent::Exited => panic!("shell exited unexpectedly"),
                }
            }
            thread::sleep(Duration::from_millis(10));
        }
        panic!("timed out waiting for Bash result; output={output:?}");
    }
}
