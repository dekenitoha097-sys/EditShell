use editshell::config::{Settings, Theme};
use editshell::shell::{SessionEvent, ShellSession, bash_quote};
use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;
use gtk4 as gtk;
use std::cell::{Cell, RefCell};
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

struct TabState {
    session: ShellSession,
    cwd: PathBuf,
    editor: gtk::TextView,
    output: gtk::TextView,
    status: gtk::Label,
    run_count: u64,
    output_tail: String,
    password_dialog_open: bool,
    command_history: Vec<String>,
    history_position: Option<usize>,
    history_draft: String,
}

type SharedTab = Rc<RefCell<TabState>>;

pub fn run() {
    let app = gtk::Application::builder()
        .application_id("io.github.editshell.EditShell")
        .build();
    app.connect_activate(build_window);
    app.run();
}

fn build_window(app: &gtk::Application) {
    let settings = Rc::new(RefCell::new(Settings::load()));
    let tabs: Rc<RefCell<Vec<SharedTab>>> = Rc::new(RefCell::new(Vec::new()));
    let active_tab: Rc<RefCell<Option<SharedTab>>> = Rc::new(RefCell::new(None));

    apply_theme(settings.borrow().theme);

    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title("EditShell")
        .default_width(1180)
        .default_height(760)
        .build();

    let header = gtk::HeaderBar::new();
    let title = gtk::Label::new(Some("EditShell"));
    title.add_css_class("title");
    header.set_title_widget(Some(&title));
    let new_tab = gtk::Button::from_icon_name("tab-new-symbolic");
    new_tab.set_tooltip_text(Some("Nouvel onglet"));
    header.pack_start(&new_tab);
    let explorer_toggle = gtk::ToggleButton::with_label("Fichiers");
    explorer_toggle.set_active(true);
    header.pack_start(&explorer_toggle);
    let preferences = gtk::Button::from_icon_name("preferences-system-symbolic");
    preferences.set_tooltip_text(Some("Préférences"));
    header.pack_end(&preferences);
    window.set_titlebar(Some(&header));

    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let paned = gtk::Paned::new(gtk::Orientation::Horizontal);
    paned.set_position(260);

    let explorer = build_explorer(Rc::clone(&active_tab));
    paned.set_start_child(Some(&explorer.panel));

    let notebook = gtk::Notebook::new();
    notebook.set_scrollable(true);
    paned.set_end_child(Some(&notebook));
    root.append(&paned);

    let footer = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    footer.add_css_class("toolbar");
    footer.set_margin_start(12);
    footer.set_margin_end(12);
    footer.set_margin_top(6);
    footer.set_margin_bottom(6);
    let cwd_label = gtk::Label::new(Some("Répertoire : —"));
    cwd_label.set_xalign(0.0);
    cwd_label.set_hexpand(true);
    footer.append(&cwd_label);
    let shell_label = gtk::Label::new(Some("Bash"));
    shell_label.add_css_class("dim-label");
    footer.append(&shell_label);
    root.append(&footer);
    window.set_child(Some(&root));

    let explorer_panel = explorer.panel.clone();
    explorer_toggle.connect_toggled(move |button| {
        explorer_panel.set_visible(button.is_active());
    });

    let notebook_for_new = notebook.clone();
    let tabs_for_new = Rc::clone(&tabs);
    let active_for_new = Rc::clone(&active_tab);
    let cwd_for_new = cwd_label.clone();
    let explorer_root = explorer.set_root.clone();
    let window_for_new_tab = window.clone();
    new_tab.connect_clicked(move |_| {
        add_tab(
            &notebook_for_new,
            &tabs_for_new,
            &active_for_new,
            &cwd_for_new,
            &explorer_root,
            &window_for_new_tab,
            None,
        );
    });

    let tabs_for_switch = Rc::clone(&tabs);
    let active_for_switch = Rc::clone(&active_tab);
    let cwd_for_switch = cwd_label.clone();
    let root_for_switch = explorer.set_root.clone();
    notebook.connect_switch_page(move |_, _, page_num| {
        let tab = tabs_for_switch.borrow().get(page_num as usize).cloned();
        if let Some(tab) = tab {
            let cwd = tab.borrow().cwd.clone();
            cwd_for_switch.set_text(&format!("Répertoire : {}", cwd.display()));
            *active_for_switch.borrow_mut() = Some(tab);
            root_for_switch(cwd);
        }
    });

    let parent_window = window.clone();
    let settings_for_dialog = Rc::clone(&settings);
    preferences.connect_clicked(move |_| {
        show_preferences(&parent_window, &settings_for_dialog);
    });

    install_css(settings.borrow().font_size);
    add_tab(
        &notebook,
        &tabs,
        &active_tab,
        &cwd_label,
        &explorer.set_root,
        &window,
        None,
    );
    window.present();
}

struct Explorer {
    panel: gtk::Widget,
    set_root: Rc<dyn Fn(PathBuf)>,
}

fn build_explorer(active_tab: Rc<RefCell<Option<SharedTab>>>) -> Explorer {
    let panel = gtk::Box::new(gtk::Orientation::Vertical, 6);
    panel.set_size_request(190, -1);
    panel.set_margin_top(8);
    panel.set_margin_start(8);
    panel.set_margin_end(8);
    panel.set_margin_bottom(8);

    let heading = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let up_button = gtk::Button::from_icon_name("go-up-symbolic");
    up_button.set_tooltip_text(Some("Dossier parent"));
    let home_button = gtk::Button::from_icon_name("go-home-symbolic");
    home_button.set_tooltip_text(Some("Dossier personnel"));
    let location = gtk::Label::new(Some("Fichiers"));
    location.set_xalign(0.0);
    location.set_hexpand(true);
    location.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    location.add_css_class("heading");
    let refresh_button = gtk::Button::from_icon_name("view-refresh-symbolic");
    refresh_button.set_tooltip_text(Some("Actualiser"));
    heading.append(&up_button);
    heading.append(&home_button);
    heading.append(&location);
    heading.append(&refresh_button);
    panel.append(&heading);

    let insert_button = gtk::Button::with_label("Insérer le chemin");
    insert_button.set_sensitive(false);
    panel.append(&insert_button);

    let list = gtk::ListBox::new();
    list.set_selection_mode(gtk::SelectionMode::Single);
    list.add_css_class("navigation-sidebar");
    let scroller = gtk::ScrolledWindow::new();
    scroller.set_vexpand(true);
    scroller.set_child(Some(&list));
    panel.append(&scroller);

    let current_path = Rc::new(RefCell::new(
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")),
    ));
    let entries: Rc<RefCell<Vec<PathBuf>>> = Rc::new(RefCell::new(Vec::new()));
    let selected_path: Rc<RefCell<Option<PathBuf>>> = Rc::new(RefCell::new(None));
    let refresh_fn: Rc<dyn Fn()> = {
        let list = list.clone();
        let location = location.clone();
        let current_path = Rc::clone(&current_path);
        let entries = Rc::clone(&entries);
        let selected_path = Rc::clone(&selected_path);
        let insert_button = insert_button.clone();
        Rc::new(move || {
            while let Some(child) = list.first_child() {
                list.remove(&child);
            }
            *selected_path.borrow_mut() = None;
            insert_button.set_sensitive(false);
            let path = current_path.borrow().clone();
            location.set_text(
                path.file_name()
                    .unwrap_or(path.as_os_str())
                    .to_string_lossy()
                    .as_ref(),
            );
            location.set_tooltip_text(Some(&path.display().to_string()));
            let mut children = match fs::read_dir(&path) {
                Ok(items) => items
                    .filter_map(Result::ok)
                    .map(|entry| entry.path())
                    .collect::<Vec<_>>(),
                Err(error) => {
                    list.append(&gtk::Label::new(Some(&format!(
                        "Impossible de lire : {error}"
                    ))));
                    entries.borrow_mut().clear();
                    return;
                }
            };
            children.sort_by_key(|item| {
                (
                    !item.is_dir(),
                    item.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_lowercase(),
                )
            });
            entries.borrow_mut().clone_from(&children);
            for child_path in children {
                let name = child_path
                    .file_name()
                    .unwrap_or(child_path.as_os_str())
                    .to_string_lossy();
                let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                let icon = gtk::Image::from_icon_name(if child_path.is_dir() {
                    "folder-symbolic"
                } else {
                    "text-x-generic-symbolic"
                });
                let row_label = gtk::Label::new(Some(&name));
                row_label.set_xalign(0.0);
                row_label.set_hexpand(true);
                row_label.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
                icon.set_pixel_size(16);
                row.append(&icon);
                row.append(&row_label);
                row.set_margin_start(8);
                row.set_margin_end(8);
                row_label.set_margin_top(5);
                row_label.set_margin_bottom(5);
                list.append(&row);
            }
        })
    };
    let set_root: Rc<dyn Fn(PathBuf)> = {
        let current_path = Rc::clone(&current_path);
        let refresh = Rc::clone(&refresh_fn);
        Rc::new(move |path| {
            *current_path.borrow_mut() = path;
            refresh();
        })
    };

    {
        let current_path = Rc::clone(&current_path);
        let set_root = Rc::clone(&set_root);
        up_button.connect_clicked(move |_| {
            let parent = current_path
                .borrow()
                .parent()
                .map(PathBuf::from)
                .unwrap_or_else(|| current_path.borrow().clone());
            set_root(parent);
        });
    }
    {
        let set_root = Rc::clone(&set_root);
        home_button.connect_clicked(move |_| {
            if let Some(home) = std::env::var_os("HOME") {
                set_root(PathBuf::from(home));
            }
        });
    }

    {
        let refresh = Rc::clone(&refresh_fn);
        refresh_button.connect_clicked(move |_| refresh());
    }
    {
        let entries = Rc::clone(&entries);
        let selected_path = Rc::clone(&selected_path);
        let insert_button = insert_button.clone();
        list.connect_row_selected(move |_, row| {
            *selected_path.borrow_mut() =
                row.and_then(|row| entries.borrow().get(row.index() as usize).cloned());
            insert_button.set_sensitive(selected_path.borrow().is_some());
        });
    }
    {
        let selected_path = Rc::clone(&selected_path);
        let active_tab = Rc::clone(&active_tab);
        insert_button.connect_clicked(move |_| {
            if let Some(path) = selected_path.borrow().clone() {
                insert_path_into_editor(&active_tab, &path);
            }
        });
    }
    {
        let entries = Rc::clone(&entries);
        let current_path = Rc::clone(&current_path);
        let refresh = Rc::clone(&refresh_fn);
        let active_tab = Rc::clone(&active_tab);
        list.connect_row_activated(move |_, row| {
            let Some(path) = entries.borrow().get(row.index() as usize).cloned() else {
                return;
            };
            if path.is_dir() {
                *current_path.borrow_mut() = path;
                refresh();
            } else {
                insert_path_into_editor(&active_tab, &path);
            }
        });
    }
    refresh_fn();

    Explorer {
        panel: panel.upcast(),
        set_root,
    }
}

fn insert_path_into_editor(active_tab: &Rc<RefCell<Option<SharedTab>>>, path: &Path) {
    let Some(tab) = active_tab.borrow().clone() else {
        return;
    };
    let editor = tab.borrow().editor.clone();
    editor
        .buffer()
        .insert_at_cursor(&format!("{} ", bash_quote(&path.to_string_lossy())));
    editor.grab_focus();
}

fn is_password_prompt(text: &str) -> bool {
    let text = text.to_lowercase();
    let trimmed = text.trim_end();
    trimmed.contains("[sudo: authenticate]")
        || (trimmed.ends_with(':')
            && (trimmed.contains("password")
                || trimmed.contains("passphrase")
                || trimmed.contains("mot de passe")))
}

fn show_password_prompt(window: &gtk::ApplicationWindow, tab: &SharedTab) {
    let dialog = gtk::Dialog::builder()
        .transient_for(window)
        .modal(true)
        .title("Authentification requise")
        .build();
    dialog.add_button("Annuler", gtk::ResponseType::Cancel);
    dialog.add_button("Envoyer", gtk::ResponseType::Accept);
    dialog.set_default_response(gtk::ResponseType::Accept);

    let content = dialog.content_area();
    content.set_spacing(12);
    content.set_margin_top(16);
    content.set_margin_bottom(16);
    content.set_margin_start(16);
    content.set_margin_end(16);
    let explanation = gtk::Label::new(Some(
        "Cette commande demande un mot de passe. La saisie est masquée et n’est pas ajoutée à l’historique.",
    ));
    explanation.set_wrap(true);
    explanation.set_xalign(0.0);
    content.append(&explanation);
    let password = gtk::PasswordEntry::new();
    password.set_placeholder_text(Some("Mot de passe"));
    password.set_show_peek_icon(false);
    password.set_activates_default(true);
    content.append(&password);

    let tab = Rc::clone(tab);
    let password_for_response = password.clone();
    dialog.connect_response(move |dialog, response| {
        let result = if response == gtk::ResponseType::Accept {
            let password = password_for_response.text();
            tab.borrow().session.send_input(password.as_str())
        } else {
            tab.borrow().session.interrupt()
        };
        password_for_response.set_text("");
        {
            let mut state = tab.borrow_mut();
            state.password_dialog_open = false;
            match result {
                Ok(()) if response == gtk::ResponseType::Accept => {
                    state.status.set_text("Réponse d’authentification envoyée.");
                }
                Ok(()) => state.status.set_text("Commande interrompue."),
                Err(error) => state.status.set_text(&format!(
                    "Impossible de traiter l’authentification : {error}"
                )),
            }
        }
        dialog.set_visible(false);
    });
    dialog.present();
    password.grab_focus();
}

fn add_tab(
    notebook: &gtk::Notebook,
    tabs: &Rc<RefCell<Vec<SharedTab>>>,
    active_tab: &Rc<RefCell<Option<SharedTab>>>,
    cwd_label: &gtk::Label,
    set_explorer_root: &Rc<dyn Fn(PathBuf)>,
    parent_window: &gtk::ApplicationWindow,
    start_dir: Option<PathBuf>,
) {
    let cwd = start_dir
        .or_else(|| {
            active_tab
                .borrow()
                .as_ref()
                .map(|tab| tab.borrow().cwd.clone())
        })
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("/"));
    let session = match ShellSession::start(Some(cwd.clone())) {
        Ok(session) => session,
        Err(error) => {
            eprintln!("Impossible de démarrer Bash : {error}");
            return;
        }
    };

    let editor = gtk::TextView::new();
    editor.set_monospace(true);
    editor.set_wrap_mode(gtk::WrapMode::None);
    editor.set_vexpand(true);
    editor.set_top_margin(12);
    editor.set_bottom_margin(12);
    editor.set_left_margin(14);
    editor.set_right_margin(14);
    editor.add_css_class("shell-editor");
    editor.set_tooltip_text(Some(
        "Éditez librement vos commandes. Ctrl+Entrée les exécute.",
    ));

    let output = gtk::TextView::new();
    output.set_editable(false);
    output.set_cursor_visible(false);
    output.set_monospace(true);
    output.set_wrap_mode(gtk::WrapMode::WordChar);
    output.set_size_request(-1, 180);
    output.set_left_margin(14);
    output.set_right_margin(14);
    output.set_top_margin(8);
    output.set_bottom_margin(8);
    output.add_css_class("shell-output");

    let status = gtk::Label::new(Some("Prêt — Ctrl+Entrée pour exécuter"));
    status.set_xalign(0.0);
    status.set_margin_start(14);
    status.set_margin_end(14);
    status.set_margin_top(8);
    status.add_css_class("dim-label");

    let run_button = gtk::Button::with_label("Exécuter");
    run_button.add_css_class("suggested-action");
    let stop_button = gtk::Button::from_icon_name("process-stop-symbolic");
    stop_button.set_tooltip_text(Some("Interrompre la commande (Ctrl+C dans la sortie)"));
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    actions.set_margin_start(10);
    actions.set_margin_end(10);
    actions.set_margin_top(8);
    actions.set_margin_bottom(4);
    let hint = gtk::Label::new(Some("Édition libre · Ctrl+Entrée"));
    hint.set_xalign(0.0);
    hint.set_hexpand(true);
    hint.add_css_class("dim-label");
    actions.append(&hint);
    actions.append(&run_button);
    actions.append(&stop_button);

    let output_scroll = gtk::ScrolledWindow::new();
    output_scroll.set_min_content_height(300);
    output_scroll.set_vexpand(true);
    output_scroll.set_child(Some(&output));
    let history_heading = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    history_heading.set_margin_start(12);
    history_heading.set_margin_end(12);
    history_heading.set_margin_top(8);
    history_heading.set_margin_bottom(4);
    let history_title = gtk::Label::new(Some("Historique des commandes et résultats"));
    history_title.set_xalign(0.0);
    history_title.set_hexpand(true);
    history_title.add_css_class("heading");
    history_heading.append(&history_title);

    let interactive_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    interactive_row.add_css_class("toolbar");
    interactive_row.set_margin_start(10);
    interactive_row.set_margin_end(10);
    interactive_row.set_margin_top(6);
    interactive_row.set_margin_bottom(8);
    let interactive_label = gtk::Label::new(Some("Session active :"));
    interactive_label.add_css_class("dim-label");
    interactive_row.append(&interactive_label);
    let interactive_input = gtk::Entry::new();
    interactive_input.set_hexpand(true);
    interactive_input
        .set_placeholder_text(Some("Réponse ou commande distante (ex. SSH, FTP, SMB)…"));
    interactive_input.set_sensitive(false);
    let send_input = gtk::Button::with_label("Envoyer ↵");
    send_input.set_sensitive(false);
    interactive_row.append(&interactive_input);
    interactive_row.append(&send_input);

    let editor_area = gtk::Box::new(gtk::Orientation::Vertical, 0);
    editor_area.set_size_request(-1, 220);
    editor_area.append(&actions);
    let editor_scroll = gtk::ScrolledWindow::new();
    editor_scroll.set_min_content_height(220);
    editor_scroll.set_vexpand(true);
    editor_scroll.set_child(Some(&editor));
    editor_area.append(&editor_scroll);

    let history_area = gtk::Box::new(gtk::Orientation::Vertical, 0);
    history_area.set_size_request(-1, 300);
    let separator = gtk::Separator::new(gtk::Orientation::Horizontal);
    history_area.append(&separator);
    history_area.append(&status);
    history_area.append(&history_heading);
    history_area.append(&output_scroll);
    history_area.append(&interactive_row);

    let workspace = gtk::Paned::new(gtk::Orientation::Vertical);
    workspace.set_wide_handle(true);
    workspace.set_resize_start_child(true);
    workspace.set_resize_end_child(true);
    workspace.set_shrink_start_child(false);
    workspace.set_shrink_end_child(false);
    workspace.set_start_child(Some(&editor_area));
    workspace.set_end_child(Some(&history_area));
    workspace.set_position(300);

    let tab = Rc::new(RefCell::new(TabState {
        session,
        cwd: cwd.clone(),
        editor: editor.clone(),
        output: output.clone(),
        status: status.clone(),
        run_count: 0,
        output_tail: String::new(),
        password_dialog_open: false,
        command_history: Vec::new(),
        history_position: None,
        history_draft: String::new(),
    }));
    let history_editing = Rc::new(Cell::new(false));

    {
        let tab_weak = Rc::downgrade(&tab);
        let cwd_label = cwd_label.clone();
        let active_tab = Rc::clone(active_tab);
        let set_explorer_root = Rc::clone(set_explorer_root);
        let parent_window = parent_window.clone();
        let output_scroll = output_scroll.clone();
        let interactive_input = interactive_input.clone();
        let send_input = send_input.clone();
        let interactive_label = interactive_label.clone();
        glib::timeout_add_local(Duration::from_millis(40), move || {
            let Some(tab) = tab_weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            let mut state = tab.borrow_mut();
            let mut password_prompt = false;
            while let Some(event) = state.session.try_event() {
                match event {
                    SessionEvent::Output(text) => {
                        let candidate = format!("{}{}", state.output_tail, text);
                        state.output_tail = candidate.chars().rev().take(256).collect::<String>();
                        state.output_tail = state.output_tail.chars().rev().collect();
                        if is_password_prompt(&candidate) {
                            password_prompt = true;
                            state.output_tail.clear();
                        }
                        let buffer = state.output.buffer();
                        let mut end = buffer.end_iter();
                        buffer.insert(&mut end, &text);
                        scroll_output_to_bottom(&output_scroll);
                    }
                    SessionEvent::ClearOutput => {
                        state.output.buffer().set_text("");
                    }
                    SessionEvent::Finished { status, cwd } => {
                        state.cwd = cwd;
                        state.password_dialog_open = false;
                        if active_tab
                            .borrow()
                            .as_ref()
                            .is_some_and(|active| Rc::ptr_eq(active, &tab))
                        {
                            cwd_label.set_text(&format!("Répertoire : {}", state.cwd.display()));
                            set_explorer_root(state.cwd.clone());
                        }
                        if status == 0 {
                            state.status.set_text("Terminé · code de retour 0");
                        } else {
                            state
                                .status
                                .set_text(&format!("Échec · code de retour {status}"));
                        }
                        let output_buffer = state.output.buffer();
                        let mut end = output_buffer.end_iter();
                        output_buffer
                            .insert(&mut end, &format!("\n[code de retour : {status}]\n\n"));
                        scroll_output_to_bottom(&output_scroll);
                    }
                    SessionEvent::Failed(error) => {
                        state.password_dialog_open = false;
                        state.status.set_text(&error);
                        let output = state.output.buffer();
                        let mut end = output.end_iter();
                        output.insert(&mut end, &format!("\n[erreur : {error}]\n"));
                        scroll_output_to_bottom(&output_scroll);
                    }
                }
            }
            let running = state.session.is_running();
            interactive_input.set_sensitive(running);
            send_input.set_sensitive(running);
            interactive_label.set_text(if running {
                "Commande active :"
            } else {
                "Session active :"
            });
            interactive_input.set_placeholder_text(Some(if running {
                "Réponse ou commande distante ; Ctrl+C interrompt la commande…"
            } else {
                "Les réponses SSH/FTP/SMB apparaîtront ici pendant l’exécution…"
            }));
            if password_prompt && !state.password_dialog_open && state.session.is_running() {
                state.password_dialog_open = true;
                let tab = Rc::clone(&tab);
                drop(state);
                show_password_prompt(&parent_window, &tab);
            }
            glib::ControlFlow::Continue
        });
    }

    let execute: Rc<dyn Fn()> = {
        let tab = Rc::clone(&tab);
        let output_scroll = output_scroll.clone();
        let history_editing = Rc::clone(&history_editing);
        Rc::new(move || {
            let (command, result) = {
                let state = tab.borrow();
                let buffer = state.editor.buffer();
                let command = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false);
                let result = state.session.execute(command.as_str());
                (command, result)
            };
            match result {
                Ok(()) => {
                    let mut state = tab.borrow_mut();
                    if !command.trim().is_empty() {
                        state.command_history.push(command.to_string());
                    }
                    state.history_position = None;
                    state.history_draft.clear();
                    state.run_count += 1;
                    let timestamp = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    let output = state.output.buffer();
                    let mut end = output.end_iter();
                    output.insert(
                        &mut end,
                        &format!(
                            "\n[exécution #{} · {timestamp}]\n$ {}\n",
                            state.run_count,
                            command.as_str()
                        ),
                    );
                    history_editing.set(true);
                    state.editor.buffer().set_text("");
                    history_editing.set(false);
                    state.status.set_text(
                        "Au premier plan — répondez ci-dessous ; « + » ouvre un shell local séparé.",
                    );
                    scroll_output_to_bottom(&output_scroll);
                }
                Err(message) => tab.borrow().status.set_text(message),
            }
        })
    };
    {
        let execute = Rc::clone(&execute);
        run_button.connect_clicked(move |_| execute());
    }
    {
        let tab = Rc::clone(&tab);
        stop_button.connect_clicked(move |_| {
            let tab = tab.borrow();
            if let Err(error) = tab.session.interrupt() {
                tab.status
                    .set_text(&format!("Impossible d’interrompre la commande : {error}"));
            } else if tab.session.is_running() {
                tab.status.set_text("Interruption demandée…");
            }
        });
    }
    let submit_interactive_input: Rc<dyn Fn()> = {
        let tab = Rc::clone(&tab);
        let interactive_input = interactive_input.clone();
        Rc::new(move || {
            let input = interactive_input.text();
            match tab.borrow().session.send_input(input.as_str()) {
                Ok(()) => {
                    interactive_input.set_text("");
                    tab.borrow().status.set_text("Réponse envoyée.");
                }
                Err(error) => tab
                    .borrow()
                    .status
                    .set_text(&format!("Impossible d’envoyer la réponse : {error}")),
            }
        })
    };
    {
        let submit = Rc::clone(&submit_interactive_input);
        send_input.connect_clicked(move |_| submit());
    }
    {
        let submit = Rc::clone(&submit_interactive_input);
        interactive_input.connect_activate(move |_| submit());
    }
    {
        let tab = Rc::clone(&tab);
        let controller = gtk::EventControllerKey::new();
        controller.connect_key_pressed(move |_, key, _, modifiers| {
            if (key == gdk::Key::c || key == gdk::Key::C)
                && modifiers.contains(gdk::ModifierType::CONTROL_MASK)
            {
                let tab = tab.borrow();
                let buffer = tab.output.buffer();
                if buffer.has_selection() {
                    return glib::Propagation::Proceed;
                }
                if tab.session.is_running() {
                    match tab.session.interrupt() {
                        Ok(()) => tab.status.set_text("Interruption demandée…"),
                        Err(error) => tab
                            .status
                            .set_text(&format!("Impossible d’interrompre la commande : {error}")),
                    }
                    return glib::Propagation::Stop;
                }
            }
            glib::Propagation::Proceed
        });
        output.add_controller(controller);
    }
    {
        let execute = Rc::clone(&execute);
        let controller = gtk::EventControllerKey::new();
        controller.connect_key_pressed(move |_, key, _, modifiers| {
            if (key == gdk::Key::Return || key == gdk::Key::KP_Enter)
                && modifiers.contains(gdk::ModifierType::CONTROL_MASK)
            {
                execute();
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        editor.add_controller(controller);
    }
    {
        let tab = Rc::clone(&tab);
        let history_editing = Rc::clone(&history_editing);
        editor.buffer().connect_changed(move |_| {
            if !history_editing.get() {
                let mut state = tab.borrow_mut();
                state.history_position = None;
                state.history_draft.clear();
            }
        });
    }
    {
        let tab = Rc::clone(&tab);
        let history_editing = Rc::clone(&history_editing);
        let controller = gtk::EventControllerKey::new();
        controller.connect_key_pressed(move |_, key, _, modifiers| {
            if !modifiers.contains(gdk::ModifierType::CONTROL_MASK) {
                return glib::Propagation::Proceed;
            }

            let older = key == gdk::Key::Up;
            if !older && key != gdk::Key::Down {
                return glib::Propagation::Proceed;
            }

            let mut state = tab.borrow_mut();
            if state.command_history.is_empty() {
                return glib::Propagation::Proceed;
            }

            let history_len = state.command_history.len();
            let next_position = if older {
                match state.history_position {
                    Some(position) => position.saturating_sub(1),
                    None => {
                        let buffer = state.editor.buffer();
                        state.history_draft = buffer
                            .text(&buffer.start_iter(), &buffer.end_iter(), false)
                            .to_string();
                        history_len - 1
                    }
                }
            } else {
                match state.history_position {
                    Some(position) if position + 1 < history_len => position + 1,
                    Some(_) => {
                        let draft = std::mem::take(&mut state.history_draft);
                        state.history_position = None;
                        history_editing.set(true);
                        let buffer = state.editor.buffer();
                        buffer.set_text(&draft);
                        let end = buffer.end_iter();
                        buffer.place_cursor(&end);
                        history_editing.set(false);
                        return glib::Propagation::Stop;
                    }
                    None => return glib::Propagation::Proceed,
                }
            };

            state.history_position = Some(next_position);
            let command = state.command_history[next_position].clone();
            history_editing.set(true);
            let buffer = state.editor.buffer();
            buffer.set_text(&command);
            let end = buffer.end_iter();
            buffer.place_cursor(&end);
            history_editing.set(false);
            glib::Propagation::Stop
        });
        editor.add_controller(controller);
    }

    let page_title = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let tab_label = gtk::Label::new(Some("Shell"));
    let close = gtk::Button::from_icon_name("window-close-symbolic");
    close.set_has_frame(false);
    close.set_tooltip_text(Some("Fermer l’onglet"));
    page_title.append(&tab_label);
    page_title.append(&close);
    let page_num = notebook.append_page(&workspace, Some(&page_title));
    tabs.borrow_mut().push(Rc::clone(&tab));
    *active_tab.borrow_mut() = Some(Rc::clone(&tab));
    notebook.set_current_page(Some(page_num));
    cwd_label.set_text(&format!("Répertoire : {}", cwd.display()));
    set_explorer_root(cwd.clone());

    {
        let notebook = notebook.clone();
        let tabs = Rc::clone(tabs);
        let active_tab = Rc::clone(active_tab);
        let tab = Rc::clone(&tab);
        let cwd_label = cwd_label.clone();
        let set_explorer_root = Rc::clone(set_explorer_root);
        close.connect_clicked(move |_| {
            let Some(page) = notebook.page_num(&workspace) else {
                return;
            };
            if tabs.borrow().len() <= 1 {
                return;
            }
            if tab.borrow().session.is_running() {
                tab.borrow()
                    .status
                    .set_text("Terminez la commande avant de fermer cet onglet.");
                return;
            }
            notebook.remove_page(Some(page));
            let mut tabs = tabs.borrow_mut();
            tabs.remove(page as usize);
            let selected = notebook.current_page().unwrap_or(0) as usize;
            if let Some(tab) = tabs.get(selected).cloned() {
                cwd_label.set_text(&format!("Répertoire : {}", tab.borrow().cwd.display()));
                *active_tab.borrow_mut() = Some(tab);
                if let Some(tab) = tabs.get(selected) {
                    set_explorer_root(tab.borrow().cwd.clone());
                }
            }
        });
    }
}

fn show_preferences(window: &gtk::ApplicationWindow, settings: &Rc<RefCell<Settings>>) {
    let dialog = gtk::Dialog::builder()
        .transient_for(window)
        .modal(true)
        .title("Préférences")
        .build();
    dialog.add_button("Annuler", gtk::ResponseType::Cancel);
    dialog.add_button("Enregistrer", gtk::ResponseType::Accept);

    let content = dialog.content_area();
    content.set_spacing(12);
    content.set_margin_top(16);
    content.set_margin_bottom(16);
    content.set_margin_start(16);
    content.set_margin_end(16);

    let theme_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let theme_label = gtk::Label::new(Some("Thème"));
    theme_label.set_xalign(0.0);
    theme_label.set_hexpand(true);
    let theme = gtk::DropDown::from_strings(&["Système", "Clair", "Sombre"]);
    theme.set_selected(match settings.borrow().theme {
        Theme::System => 0,
        Theme::Light => 1,
        Theme::Dark => 2,
    });
    theme_row.append(&theme_label);
    theme_row.append(&theme);
    content.append(&theme_row);

    let font_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let font_label = gtk::Label::new(Some("Taille de la police"));
    font_label.set_xalign(0.0);
    font_label.set_hexpand(true);
    let font_size = gtk::SpinButton::with_range(8.0, 32.0, 1.0);
    font_size.set_value(f64::from(settings.borrow().font_size));
    font_row.append(&font_label);
    font_row.append(&font_size);
    content.append(&font_row);

    let settings = Rc::clone(settings);
    dialog.connect_response(move |dialog, response| {
        if response == gtk::ResponseType::Accept {
            let next = Settings {
                theme: match theme.selected() {
                    1 => Theme::Light,
                    2 => Theme::Dark,
                    _ => Theme::System,
                },
                font_size: font_size.value_as_int().clamp(8, 32) as u8,
            };
            match next.save() {
                Ok(()) => {
                    apply_theme(next.theme);
                    install_css(next.font_size);
                    *settings.borrow_mut() = next;
                }
                Err(error) => eprintln!("Impossible d’enregistrer les préférences : {error}"),
            }
        }
        dialog.close();
    });
    dialog.present();
}

fn scroll_output_to_bottom(scrolled: &gtk::ScrolledWindow) {
    let adjustment = scrolled.vadjustment();
    glib::idle_add_local_once(move || {
        let bottom = (adjustment.upper() - adjustment.page_size()).max(adjustment.lower());
        adjustment.set_value(bottom);
    });
}

fn apply_theme(theme: Theme) {
    if let Some(settings) = gtk::Settings::default() {
        settings.set_gtk_application_prefer_dark_theme(theme == Theme::Dark);
    }
}

fn install_css(font_size: u8) {
    let Some(display) = gdk::Display::default() else {
        return;
    };
    let provider = gtk::CssProvider::new();
    provider.load_from_data(&format!(
        ".shell-editor, .shell-output {{ font-family: monospace; font-size: {font_size}pt; }}\n\
         .shell-output {{ background: alpha(@theme_fg_color, 0.035); }}\n"
    ));
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}
