use editshell::config::{OutputPalette, Settings, Theme};
use editshell::shell::{SessionEvent, ShellSession, bash_quote};
use gtk::gdk;
use gtk::glib;
use gtk4 as gtk;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::collections::HashSet;
use std::env;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use vte4::prelude::*;

struct TabState {
    session: ShellSession,
    cwd: PathBuf,
    editor: gtk::TextView,
    output: gtk::TextView,
    interactive_terminal: Option<(gtk::Window, vte4::Terminal)>,
    status: gtk::Label,
    run_count: u64,
    output_tail: String,
    password_dialog_open: bool,
    command_history: Vec<String>,
    history_position: Option<usize>,
    history_draft: String,
    terminal_style: AnsiStyle,
    output_tags: HashMap<AnsiStyle, gtk::TextTag>,
}

type SharedTab = Rc<RefCell<TabState>>;

struct TabOptions {
    settings: Rc<RefCell<Settings>>,
    start_dir: Option<PathBuf>,
}

#[derive(Clone, Debug, Default, Hash, PartialEq, Eq)]
struct AnsiStyle {
    foreground: Option<String>,
    background: Option<String>,
    bold: bool,
    underline: bool,
}

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
        .default_height(680)
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
    let settings_for_new_tab = Rc::clone(&settings);
    new_tab.connect_clicked(move |_| {
        add_tab(
            &notebook_for_new,
            &tabs_for_new,
            &active_for_new,
            &cwd_for_new,
            &explorer_root,
            &window_for_new_tab,
            TabOptions {
                settings: Rc::clone(&settings_for_new_tab),
                start_dir: None,
            },
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
        TabOptions {
            settings: Rc::clone(&settings),
            start_dir: None,
        },
    );
    window.present();
}

struct Explorer {
    panel: gtk::Widget,
    set_root: Rc<dyn Fn(PathBuf)>,
}

struct FileMenuContext {
    active_tab: Rc<RefCell<Option<SharedTab>>>,
    current_path: Rc<RefCell<PathBuf>>,
    refresh_button: gtk::Button,
    status: gtk::Label,
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

    let explorer_status =
        gtk::Label::new(Some("Clic droit sur un élément pour afficher les options."));
    explorer_status.set_xalign(0.0);
    explorer_status.set_wrap(true);
    explorer_status.add_css_class("dim-label");
    panel.append(&explorer_status);

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
    let active_tab_for_refresh = Rc::clone(&active_tab);
    let refresh_button_for_rows = refresh_button.clone();
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

                let context_click = gtk::GestureClick::new();
                context_click.set_button(3);
                let path_for_menu = child_path.clone();
                let row_for_menu = row.clone();
                let active_tab = Rc::clone(&active_tab_for_refresh);
                let current_path = Rc::clone(&current_path);
                let refresh_button = refresh_button_for_rows.clone();
                let selected_path = Rc::clone(&selected_path);
                let insert_button = insert_button.clone();
                let explorer_status = explorer_status.clone();
                let context = FileMenuContext {
                    active_tab,
                    current_path,
                    refresh_button,
                    status: explorer_status,
                };
                context_click.connect_pressed(move |_, _, x, y| {
                    *selected_path.borrow_mut() = Some(path_for_menu.clone());
                    insert_button.set_sensitive(true);
                    show_file_context_menu(&row_for_menu, &path_for_menu, x, y, &context);
                });
                row.add_controller(context_click);
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

fn show_file_context_menu(
    anchor: &impl IsA<gtk::Widget>,
    path: &Path,
    x: f64,
    y: f64,
    context: &FileMenuContext,
) {
    let FileMenuContext {
        active_tab,
        current_path,
        refresh_button,
        status,
    } = context;
    let popover = gtk::Popover::new();
    popover.set_has_arrow(true);
    popover.set_parent(anchor);
    popover.connect_closed(|popover| popover.unparent());
    popover.set_pointing_to(Some(&gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
    let menu = gtk::Box::new(gtk::Orientation::Vertical, 2);
    menu.set_margin_top(6);
    menu.set_margin_bottom(6);
    menu.set_margin_start(6);
    menu.set_margin_end(6);

    let insert = gtk::Button::with_label("Insérer le chemin dans l’éditeur");
    insert.set_halign(gtk::Align::Fill);
    {
        let path = path.to_path_buf();
        let active_tab = Rc::clone(active_tab);
        let status = status.clone();
        let popover = popover.clone();
        insert.connect_clicked(move |_| {
            insert_path_into_editor(&active_tab, &path);
            status.set_text(&format!("Chemin inséré : {}", path.display()));
            popover.popdown();
        });
    }
    menu.append(&insert);

    let copy = gtk::Button::with_label("Copier le chemin");
    copy.set_halign(gtk::Align::Fill);
    {
        let path = path.to_path_buf();
        let anchor = anchor.clone();
        let status = status.clone();
        let popover = popover.clone();
        copy.connect_clicked(move |_| {
            anchor
                .display()
                .clipboard()
                .set_text(&path.display().to_string());
            status.set_text("Chemin copié dans le presse-papiers.");
            popover.popdown();
        });
    }
    menu.append(&copy);

    if path.is_dir() {
        let browse = gtk::Button::with_label("Ouvrir ici dans l’explorateur EditShell");
        browse.set_halign(gtk::Align::Fill);
        {
            let path = path.to_path_buf();
            let current_path = Rc::clone(current_path);
            let refresh_button = refresh_button.clone();
            let status = status.clone();
            let popover = popover.clone();
            browse.connect_clicked(move |_| {
                *current_path.borrow_mut() = path.clone();
                refresh_button.emit_clicked();
                status.set_text(&format!("Dossier ouvert : {}", path.display()));
                popover.popdown();
            });
        }
        menu.append(&browse);
    }

    let open = gtk::Button::with_label(if path.is_dir() {
        "Ouvrir dans l’explorateur de fichiers"
    } else {
        "Ouvrir avec l’application par défaut"
    });
    open.set_halign(gtk::Align::Fill);
    {
        let path = path.to_path_buf();
        let status = status.clone();
        let popover = popover.clone();
        open.connect_clicked(move |_| {
            let uri = gtk::gio::File::for_path(&path).uri();
            let result = gtk::gio::AppInfo::launch_default_for_uri(
                &uri,
                None::<&gtk::gio::AppLaunchContext>,
            );
            match result {
                Ok(()) => status.set_text(&format!("Ouverture demandée : {}", path.display())),
                Err(error) => status.set_text(&format!("Impossible d’ouvrir l’élément : {error}")),
            }
            popover.popdown();
        });
    }
    menu.append(&open);
    popover.set_child(Some(&menu));
    popover.popup();
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
    options: TabOptions,
) {
    let TabOptions {
        settings,
        start_dir,
    } = options;
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
        "Éditez librement vos commandes. Tab complète les commandes et chemins ; Ctrl+Entrée exécute.",
    ));
    let prompt = gtk::Label::new(Some(&shell_prompt(&cwd)));
    prompt.set_xalign(0.0);
    prompt.set_margin_start(14);
    prompt.set_margin_top(6);
    prompt.add_css_class("shell-prompt");

    let output = gtk::TextView::new();
    output.set_editable(false);
    output.set_cursor_visible(false);
    output.set_monospace(true);
    output.set_wrap_mode(gtk::WrapMode::WordChar);
    output.set_size_request(-1, 120);
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
    let hint = gtk::Label::new(Some("Tab compléter · Ctrl+Entrée exécuter"));
    hint.set_xalign(0.0);
    hint.set_hexpand(true);
    hint.add_css_class("dim-label");
    actions.append(&hint);
    actions.append(&run_button);
    actions.append(&stop_button);

    let output_scroll = gtk::ScrolledWindow::new();
    output_scroll.set_min_content_height(140);
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
    editor_area.set_size_request(-1, 125);
    editor_area.append(&actions);
    editor_area.append(&prompt);
    let editor_scroll = gtk::ScrolledWindow::new();
    editor_scroll.set_min_content_height(80);
    editor_scroll.set_vexpand(true);
    editor_scroll.set_child(Some(&editor));
    editor_area.append(&editor_scroll);

    let history_area = gtk::Box::new(gtk::Orientation::Vertical, 0);
    history_area.set_size_request(-1, 200);
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
    workspace.set_position(160);

    let tab = Rc::new(RefCell::new(TabState {
        session,
        cwd: cwd.clone(),
        editor: editor.clone(),
        output: output.clone(),
        interactive_terminal: None,
        status: status.clone(),
        run_count: 0,
        output_tail: String::new(),
        password_dialog_open: false,
        command_history: Vec::new(),
        history_position: None,
        history_draft: String::new(),
        terminal_style: AnsiStyle::default(),
        output_tags: HashMap::new(),
    }));
    let history_editing = Rc::new(Cell::new(false));
    install_editor_highlighting(&editor.buffer());

    {
        let tab_weak = Rc::downgrade(&tab);
        let cwd_label = cwd_label.clone();
        let active_tab = Rc::clone(active_tab);
        let set_explorer_root = Rc::clone(set_explorer_root);
        let parent_window = parent_window.clone();
        let settings = Rc::clone(&settings);
        let output_scroll = output_scroll.clone();
        let interactive_input = interactive_input.clone();
        let send_input = send_input.clone();
        let interactive_label = interactive_label.clone();
        let prompt = prompt.clone();
        let notebook = notebook.clone();
        let workspace = workspace.clone();
        let tabs = Rc::clone(tabs);
        glib::timeout_add_local(Duration::from_millis(40), move || {
            let Some(tab) = tab_weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            let mut state = tab.borrow_mut();
            let mut password_prompt = false;
            let mut session_exited = false;
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
                        let preferences = settings.borrow();
                        let buffer = state.output.buffer();
                        let TabState {
                            terminal_style,
                            output_tags,
                            ..
                        } = &mut *state;
                        append_terminal_output(
                            &buffer,
                            &text,
                            preferences.colorize_output,
                            preferences.output_palette,
                            terminal_style,
                            output_tags,
                        );
                        scroll_output_to_bottom(&output_scroll);
                    }
                    SessionEvent::ClearOutput => {
                        state.output.buffer().set_text("");
                        state.terminal_style = AnsiStyle::default();
                    }
                    SessionEvent::TerminalScreenStarted => {
                        if state.interactive_terminal.is_none() {
                            state.interactive_terminal =
                                Some(show_interactive_terminal(&parent_window, &tab));
                        }
                    }
                    SessionEvent::TerminalOutput(bytes) => {
                        if let Some((_, terminal)) = &state.interactive_terminal {
                            terminal.feed(&bytes);
                        }
                    }
                    SessionEvent::TerminalScreenEnded => {
                        if let Some((window, _)) = state.interactive_terminal.take() {
                            window.set_visible(false);
                        }
                    }
                    SessionEvent::Finished { status, cwd } => {
                        state.cwd = cwd;
                        state.password_dialog_open = false;
                        if let Some((window, _)) = state.interactive_terminal.take() {
                            window.set_visible(false);
                        }
                        prompt.set_text(&shell_prompt(&state.cwd));
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
                        if let Some((window, _)) = state.interactive_terminal.take() {
                            window.set_visible(false);
                        }
                        state.status.set_text(&error);
                        let output = state.output.buffer();
                        let mut end = output.end_iter();
                        output.insert(&mut end, &format!("\n[erreur : {error}]\n"));
                        scroll_output_to_bottom(&output_scroll);
                    }
                    SessionEvent::Exited => {
                        if let Some((window, _)) = state.interactive_terminal.take() {
                            window.set_visible(false);
                        }
                        session_exited = true;
                        break;
                    }
                }
            }
            if session_exited {
                drop(state);
                let Some(page) = notebook.page_num(&workspace) else {
                    return glib::ControlFlow::Break;
                };
                notebook.remove_page(Some(page));
                let mut open_tabs = tabs.borrow_mut();
                if (page as usize) < open_tabs.len() {
                    open_tabs.remove(page as usize);
                }
                if open_tabs.is_empty() {
                    parent_window.close();
                    return glib::ControlFlow::Break;
                }
                let selected = notebook.current_page().unwrap_or(0) as usize;
                if let Some(selected_tab) = open_tabs.get(selected).cloned() {
                    let cwd = selected_tab.borrow().cwd.clone();
                    cwd_label.set_text(&format!("Répertoire : {}", cwd.display()));
                    *active_tab.borrow_mut() = Some(selected_tab);
                    set_explorer_root(cwd);
                }
                return glib::ControlFlow::Break;
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
    {
        let tab = Rc::clone(&tab);
        let controller = gtk::EventControllerKey::new();
        controller.connect_key_pressed(move |_, key, _, _| {
            if key != gdk::Key::Tab {
                return glib::Propagation::Proceed;
            }
            let state = tab.borrow();
            let buffer = state.editor.buffer();
            let cwd = state.cwd.clone();
            let status = state.status.clone();
            drop(state);
            let text = buffer
                .text(&buffer.start_iter(), &buffer.end_iter(), false)
                .to_string();
            let completion = completion_at_cursor(&buffer, &text, &cwd);
            match completion {
                Some(completion) if completion.candidates.len() == 1 => {
                    replace_editor_range(
                        &buffer,
                        completion.start,
                        completion.end,
                        &completion.candidates[0],
                    );
                    status.set_text("Suggestion complétée.");
                }
                Some(completion) if completion.common_prefix.len() > completion.prefix.len() => {
                    replace_editor_range(
                        &buffer,
                        completion.start,
                        completion.end,
                        &completion.common_prefix,
                    );
                    status.set_text(&format!(
                        "{} suggestions — appuyez encore sur Tab pour les afficher.",
                        completion.candidates.len()
                    ));
                }
                Some(completion) => {
                    let suggestions = completion
                        .candidates
                        .iter()
                        .take(8)
                        .cloned()
                        .collect::<Vec<_>>()
                        .join("  ");
                    status.set_text(&format!(
                        "{} suggestions : {}",
                        completion.candidates.len(),
                        suggestions
                    ));
                }
                None => status.set_text("Aucune suggestion pour cette saisie."),
            }
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

    let colorize_output =
        gtk::CheckButton::with_label("Afficher les couleurs ANSI dans les résultats");
    colorize_output.set_active(settings.borrow().colorize_output);
    content.append(&colorize_output);

    let palette_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let palette_label = gtk::Label::new(Some("Palette des résultats"));
    palette_label.set_xalign(0.0);
    palette_label.set_hexpand(true);
    let palette = gtk::DropDown::from_strings(&["Standard", "Contraste élevé"]);
    palette.set_selected(match settings.borrow().output_palette {
        OutputPalette::Standard => 0,
        OutputPalette::HighContrast => 1,
    });
    palette_row.append(&palette_label);
    palette_row.append(&palette);
    content.append(&palette_row);

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
                colorize_output: colorize_output.is_active(),
                output_palette: match palette.selected() {
                    1 => OutputPalette::HighContrast,
                    _ => OutputPalette::Standard,
                },
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

fn show_interactive_terminal(
    parent: &gtk::ApplicationWindow,
    tab: &SharedTab,
) -> (gtk::Window, vte4::Terminal) {
    let terminal = vte4::Terminal::new();
    terminal.set_hexpand(true);
    terminal.set_vexpand(true);
    terminal.set_scrollback_lines(10_000);
    terminal.set_input_enabled(true);

    let window = gtk::Window::builder()
        .title("Terminal interactif")
        .transient_for(parent)
        .modal(true)
        .default_width(1100)
        .default_height(680)
        .build();
    let layout = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let heading = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    heading.set_margin_start(12);
    heading.set_margin_end(12);
    heading.set_margin_top(10);
    heading.set_margin_bottom(8);
    let title = gtk::Label::new(Some(
        "Application interactive — utilisez les flèches et Entrée",
    ));
    title.set_xalign(0.0);
    title.set_hexpand(true);
    heading.append(&title);
    let stop = gtk::Button::with_label("Interrompre");
    heading.append(&stop);
    layout.append(&heading);

    let scroller = gtk::ScrolledWindow::new();
    scroller.set_hexpand(true);
    scroller.set_vexpand(true);
    scroller.set_child(Some(&terminal));
    layout.append(&scroller);
    window.set_child(Some(&layout));

    let weak_tab = Rc::downgrade(tab);
    terminal.connect_commit(move |_, text, _| {
        if let Some(tab) = weak_tab.upgrade()
            && let Err(error) = tab.borrow().session.send_raw_input(text.as_bytes())
        {
            tab.borrow()
                .status
                .set_text(&format!("Impossible d’envoyer la touche : {error}"));
        }
    });

    let weak_tab = Rc::downgrade(tab);
    stop.connect_clicked(move |_| {
        if let Some(tab) = weak_tab.upgrade()
            && let Err(error) = tab.borrow().session.interrupt()
        {
            tab.borrow()
                .status
                .set_text(&format!("Impossible d’interrompre la commande : {error}"));
        }
    });

    let weak_tab = Rc::downgrade(tab);
    window.connect_close_request(move |_| {
        if let Some(tab) = weak_tab.upgrade()
            && let Err(error) = tab.borrow().session.send_raw_input(b"\x03")
        {
            tab.borrow()
                .status
                .set_text(&format!("Impossible d’interrompre la commande : {error}"));
        }
        glib::Propagation::Stop
    });

    window.present();
    terminal.grab_focus();
    let resize_terminal = terminal.clone();
    let resize_window = window.clone();
    let weak_tab = Rc::downgrade(tab);
    let mut previous_size = (0_i32, 0_i32);
    glib::timeout_add_local(Duration::from_millis(200), move || {
        if !resize_window.is_visible() {
            return glib::ControlFlow::Break;
        }
        let char_width = resize_terminal.char_width() as i32;
        let char_height = resize_terminal.char_height() as i32;
        if char_width <= 0 || char_height <= 0 {
            return glib::ControlFlow::Continue;
        }
        let columns = (resize_terminal.allocated_width() / char_width).clamp(2, 500);
        let rows = (resize_terminal.allocated_height() / char_height).clamp(2, 300);
        if (columns, rows) != previous_size {
            previous_size = (columns, rows);
            resize_terminal.set_size(i64::from(columns), i64::from(rows));
            if let Some(tab) = weak_tab.upgrade()
                && let Err(error) = tab
                    .borrow()
                    .session
                    .resize_terminal(columns as u16, rows as u16)
            {
                tab.borrow().status.set_text(&format!(
                    "Impossible de redimensionner le terminal : {error}"
                ));
            }
        }
        glib::ControlFlow::Continue
    });
    (window, terminal)
}

fn append_terminal_output(
    buffer: &gtk::TextBuffer,
    text: &str,
    colorize: bool,
    palette: OutputPalette,
    style: &mut AnsiStyle,
    tags: &mut HashMap<AnsiStyle, gtk::TextTag>,
) {
    if !colorize {
        *style = AnsiStyle::default();
    }
    let characters: Vec<char> = text.chars().collect();
    let mut index = 0;
    let mut segment = String::new();
    while index < characters.len() {
        if characters[index] == '\u{1b}'
            && characters.get(index + 1) == Some(&'[')
            && let Some(relative_end) = characters[index + 2..].iter().position(|ch| *ch == 'm')
        {
            insert_terminal_segment(buffer, &segment, colorize, style, tags);
            segment.clear();
            if colorize {
                let parameters: String = characters[index + 2..index + 2 + relative_end]
                    .iter()
                    .collect();
                apply_ansi_parameters(style, &parameters, palette);
            }
            index += relative_end + 3;
        } else {
            segment.push(characters[index]);
            index += 1;
        }
    }
    insert_terminal_segment(buffer, &segment, colorize, style, tags);
}

fn shell_prompt(cwd: &Path) -> String {
    let user = env::var("USER").unwrap_or_else(|_| "user".to_owned());
    let host = env::var("HOSTNAME")
        .ok()
        .or_else(|| fs::read_to_string("/etc/hostname").ok())
        .map(|host| host.trim().to_owned())
        .filter(|host| !host.is_empty())
        .unwrap_or_else(|| "localhost".to_owned());
    format!("{user}@{host}:{} $", cwd.display())
}

fn install_editor_highlighting(buffer: &gtk::TextBuffer) {
    let executables = installed_command_names();
    let colors = [
        ("shell-command", "#3fb950"),
        ("shell-invalid-command", "#ff7b72"),
        ("shell-keyword", "#ff7b72"),
        ("shell-string", "#a5d6ff"),
        ("shell-variable", "#d2a8ff"),
        ("shell-comment", "#8b949e"),
        ("shell-operator", "#f2cc60"),
    ];
    let tags: Vec<_> = colors
        .iter()
        .map(|(name, color)| {
            let tag = gtk::TextTag::builder()
                .name(*name)
                .foreground(*color)
                .build();
            buffer.tag_table().add(&tag);
            tag
        })
        .collect();
    let buffer = buffer.clone();
    buffer.connect_changed(move |buffer| {
        let executables = &executables;
        let start = buffer.start_iter();
        let end = buffer.end_iter();
        buffer.remove_all_tags(&start, &end);

        let text = buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), false)
            .to_string();
        let chars: Vec<char> = text.chars().collect();
        let mut ranges: Vec<(usize, usize, usize)> = Vec::new();
        let mut index = 0;
        let mut command_position = true;
        while index < chars.len() {
            let ch = chars[index];
            if ch == '\n' {
                command_position = true;
                index += 1;
            } else if ch.is_whitespace() {
                index += 1;
            } else if ch == '#' {
                let start = index;
                while index < chars.len() && chars[index] != '\n' {
                    index += 1;
                }
                ranges.push((start, index, 4));
            } else if ch == '\'' || ch == '"' {
                let quote = ch;
                let start = index;
                index += 1;
                while index < chars.len() {
                    if chars[index] == '\\' && quote == '"' {
                        index = (index + 2).min(chars.len());
                    } else if chars[index] == quote {
                        index += 1;
                        break;
                    } else {
                        index += 1;
                    }
                }
                ranges.push((start, index, 2));
                command_position = false;
            } else if ch == '$' {
                let start = index;
                index += 1;
                if index < chars.len() && chars[index] == '{' {
                    index += 1;
                    while index < chars.len() && chars[index] != '}' {
                        index += 1;
                    }
                    if index < chars.len() {
                        index += 1;
                    }
                } else {
                    while index < chars.len()
                        && (chars[index].is_ascii_alphanumeric() || chars[index] == '_')
                    {
                        index += 1;
                    }
                }
                ranges.push((start, index, 3));
                command_position = false;
            } else if "|&;><()".contains(ch) {
                ranges.push((index, index + 1, 5));
                command_position = matches!(ch, '|' | '&' | ';' | '(');
                index += 1;
            } else {
                let start = index;
                while index < chars.len()
                    && !chars[index].is_whitespace()
                    && !"|&;><()".contains(chars[index])
                {
                    index += 1;
                }
                let word: String = chars[start..index].iter().collect();
                if command_position {
                    let is_keyword = [
                        "if", "then", "else", "elif", "fi", "for", "while", "until", "do", "done",
                        "case", "esac", "function", "in",
                    ]
                    .contains(&word.as_str());
                    let tag = if is_keyword {
                        2
                    } else {
                        if shell_builtin(&word) || is_executable_command(&word, executables) {
                            0
                        } else {
                            1
                        }
                    };
                    ranges.push((start, index, tag));
                    command_position = matches!(
                        word.as_str(),
                        "sudo"
                            | "doas"
                            | "command"
                            | "exec"
                            | "env"
                            | "time"
                            | "nohup"
                            | "nice"
                            | "if"
                            | "then"
                            | "else"
                            | "elif"
                            | "while"
                            | "until"
                            | "!"
                    );
                }
            }
        }

        for (start, end, tag_index) in ranges {
            if start < end {
                let start = buffer.iter_at_offset(start as i32);
                let end = buffer.iter_at_offset(end as i32);
                buffer.apply_tag(&tags[tag_index], &start, &end);
            }
        }
    });
}

fn shell_builtin(command: &str) -> bool {
    matches!(
        command,
        ":" | "."
            | "alias"
            | "bg"
            | "bind"
            | "break"
            | "builtin"
            | "caller"
            | "cd"
            | "command"
            | "compgen"
            | "complete"
            | "continue"
            | "declare"
            | "dirs"
            | "disown"
            | "echo"
            | "enable"
            | "eval"
            | "exec"
            | "exit"
            | "export"
            | "false"
            | "fc"
            | "fg"
            | "getopts"
            | "hash"
            | "help"
            | "history"
            | "jobs"
            | "kill"
            | "let"
            | "local"
            | "logout"
            | "mapfile"
            | "popd"
            | "printf"
            | "pushd"
            | "pwd"
            | "read"
            | "readonly"
            | "return"
            | "set"
            | "shift"
            | "shopt"
            | "source"
            | "suspend"
            | "test"
            | "times"
            | "trap"
            | "true"
            | "type"
            | "typeset"
            | "ulimit"
            | "umask"
            | "unalias"
            | "unset"
            | "wait"
    )
}

fn is_executable_command(command: &str, executables: &HashSet<String>) -> bool {
    if executables.contains(command) {
        return true;
    }
    if command.contains('/') {
        let path = Path::new(command);
        return fs::metadata(path).is_ok_and(|metadata| {
            metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
        });
    }
    false
}

fn installed_command_names() -> HashSet<String> {
    let mut names = HashSet::new();
    for directory in env::split_paths(&env::var_os("PATH").unwrap_or_default()) {
        if let Ok(entries) = fs::read_dir(directory) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if fs::metadata(entry.path()).is_ok_and(|metadata| {
                    metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
                }) {
                    names.insert(name);
                }
            }
        }
    }
    names
}

struct Completion {
    start: usize,
    end: usize,
    prefix: String,
    common_prefix: String,
    candidates: Vec<String>,
}

fn completion_at_cursor(buffer: &gtk::TextBuffer, text: &str, cwd: &Path) -> Option<Completion> {
    let chars: Vec<char> = text.chars().collect();
    let cursor = buffer.cursor_position().max(0) as usize;
    let cursor = cursor.min(chars.len());
    let line_start = chars[..cursor]
        .iter()
        .rposition(|ch| *ch == '\n')
        .map_or(0, |index| index + 1);
    let token_start = shell_word_start(&chars, line_start, cursor);
    let token_end = shell_word_end(&chars, token_start, cursor);
    let raw_prefix: String = chars[token_start..cursor].iter().collect();
    let prefix = unescape_shell_word(&raw_prefix);
    let command_token = command_position(&chars[line_start..token_start]);
    let candidates = if command_token && !prefix.contains('/') {
        command_candidates(&prefix)
    } else {
        path_candidates(&prefix, cwd, quote_style(&raw_prefix))
    };
    if candidates.is_empty() {
        return None;
    }

    let common_prefix = common_prefix(&candidates);
    Some(Completion {
        start: token_start,
        end: token_end,
        prefix: raw_prefix,
        common_prefix,
        candidates,
    })
}

fn shell_word_start(chars: &[char], line_start: usize, cursor: usize) -> usize {
    let mut quote = None;
    let mut escaped = false;
    let mut start = line_start;
    for (offset, ch) in chars[line_start..cursor].iter().enumerate() {
        if escaped {
            escaped = false;
        } else if *ch == '\\' && quote != Some('\'') {
            escaped = true;
        } else if matches!(*ch, '\'' | '"') {
            if quote == Some(*ch) {
                quote = None;
            } else if quote.is_none() {
                quote = Some(*ch);
            }
        } else if ch.is_whitespace() && quote.is_none() {
            start = line_start + offset + 1;
        }
    }
    start
}

fn shell_word_end(chars: &[char], start: usize, cursor: usize) -> usize {
    let mut quote = None;
    let mut escaped = false;
    for ch in &chars[start..cursor] {
        update_shell_quote_state(*ch, &mut quote, &mut escaped);
    }
    let mut index = cursor;
    while index < chars.len() && chars[index] != '\n' {
        let ch = chars[index];
        let is_separator = ch.is_whitespace() && quote.is_none() && !escaped;
        update_shell_quote_state(ch, &mut quote, &mut escaped);
        if is_separator {
            break;
        }
        index += 1;
    }
    index
}

fn update_shell_quote_state(ch: char, quote: &mut Option<char>, escaped: &mut bool) {
    if *escaped {
        *escaped = false;
    } else if ch == '\\' && *quote != Some('\'') {
        *escaped = true;
    } else if matches!(ch, '\'' | '"') {
        if *quote == Some(ch) {
            *quote = None;
        } else if quote.is_none() {
            *quote = Some(ch);
        }
    }
}

fn quote_style(raw: &str) -> Option<char> {
    raw.chars().find(|ch| matches!(ch, '\'' | '"'))
}

fn unescape_shell_word(raw: &str) -> String {
    let mut result = String::new();
    let mut quote = None;
    let mut escaped = false;
    for ch in raw.chars() {
        if escaped {
            result.push(ch);
            escaped = false;
        } else if ch == '\\' && quote != Some('\'') {
            escaped = true;
        } else if matches!(ch, '\'' | '"') {
            if quote == Some(ch) {
                quote = None;
            } else if quote.is_none() {
                quote = Some(ch);
            } else {
                result.push(ch);
            }
        } else {
            result.push(ch);
        }
    }
    if escaped {
        result.push('\\');
    }
    result
}

fn command_position(before: &[char]) -> bool {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quote = None;
    let mut escaped = false;
    let mut command_position = true;
    for ch in before {
        if escaped {
            word.push(*ch);
            escaped = false;
        } else if *ch == '\\' && quote != Some('\'') {
            escaped = true;
        } else if matches!(*ch, '\'' | '"') {
            if quote == Some(*ch) {
                quote = None;
            } else if quote.is_none() {
                quote = Some(*ch);
            } else {
                word.push(*ch);
            }
        } else if quote.is_none() && matches!(*ch, '|' | '&' | ';' | '(' | ')') {
            word.clear();
            words.clear();
            command_position = true;
        } else if ch.is_whitespace() && quote.is_none() {
            if !word.is_empty() {
                words.push(std::mem::take(&mut word));
                if command_position {
                    command_position = words
                        .last()
                        .is_some_and(|previous| next_word_is_command(previous));
                }
            }
        } else {
            word.push(*ch);
        }
    }
    if !word.is_empty() {
        command_position = next_word_is_command(word.as_str());
    }
    command_position
}

fn next_word_is_command(word: &str) -> bool {
    matches!(
        word,
        "sudo"
            | "doas"
            | "command"
            | "exec"
            | "env"
            | "time"
            | "nohup"
            | "nice"
            | "if"
            | "then"
            | "else"
            | "elif"
            | "while"
            | "until"
            | "!"
    )
}

fn command_candidates(prefix: &str) -> Vec<String> {
    let mut names = std::collections::BTreeSet::new();
    for builtin in [
        "alias", "bg", "break", "cd", "command", "continue", "echo", "eval", "exec", "exit",
        "export", "false", "fg", "jobs", "kill", "printf", "pwd", "read", "return", "set",
        "source", "test", "true", "type", "ulimit", "umask", "unalias", "unset", "wait",
    ] {
        if builtin.starts_with(prefix) {
            names.insert(builtin.to_owned());
        }
    }
    for directory in env::split_paths(&env::var_os("PATH").unwrap_or_default()) {
        if let Ok(entries) = fs::read_dir(directory) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if !name.starts_with(prefix) {
                    continue;
                }
                if fs::metadata(entry.path()).is_ok_and(|metadata| {
                    metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
                }) {
                    names.insert(name);
                }
            }
        }
    }
    names.into_iter().map(|name| format!("{name} ")).collect()
}

fn path_candidates(prefix: &str, cwd: &Path, quote: Option<char>) -> Vec<String> {
    if prefix == "~" && env::var_os("HOME").is_some() {
        return vec!["~/".to_owned()];
    }
    let ends_with_separator = prefix.ends_with('/');
    let expanded = if prefix == "~" {
        env::var_os("HOME").map(PathBuf::from).unwrap_or_default()
    } else if let Some(home_path) = prefix.strip_prefix("~/") {
        env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| cwd.to_path_buf())
            .join(home_path)
    } else {
        PathBuf::from(prefix)
    };
    let (parent, basename) = if ends_with_separator {
        (expanded.as_path(), String::new())
    } else {
        (
            expanded.parent().unwrap_or_else(|| Path::new("")),
            expanded
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
        )
    };
    let directory = if parent.as_os_str().is_empty() {
        cwd.to_path_buf()
    } else if parent.is_absolute() {
        parent.to_path_buf()
    } else {
        cwd.join(parent)
    };
    let typed_parent = prefix
        .rfind('/')
        .map(|slash| &prefix[..=slash])
        .unwrap_or("");
    let mut candidates = Vec::new();
    if let Ok(entries) = fs::read_dir(directory) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !name.starts_with(&basename) {
                continue;
            }
            let path = format!("{typed_parent}{name}");
            let suffix = if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                "/"
            } else {
                " "
            };
            candidates.push(format!(
                "{}{suffix}",
                format_shell_word(&path, quote, suffix == "/")
            ));
        }
    }
    candidates.sort();
    candidates
}

fn format_shell_word(path: &str, quote: Option<char>, is_directory: bool) -> String {
    match quote {
        Some('"') => format!(
            "\"{}\"{}",
            path.replace('\\', "\\\\").replace('"', "\\\""),
            if is_directory { "" } else { " " }
        ),
        Some('\'') => format!(
            "'{}'{}",
            path.replace('\'', "'\\''"),
            if is_directory { "" } else { " " }
        ),
        _ => format!(
            "{}{}",
            path.chars()
                .flat_map(|ch| {
                    if ch.is_whitespace() || "|&;<>()[{}]$'\"\\!*?".contains(ch) {
                        vec!['\\', ch]
                    } else {
                        vec![ch]
                    }
                })
                .collect::<String>(),
            if is_directory { "" } else { " " }
        ),
    }
}

fn common_prefix(candidates: &[String]) -> String {
    let Some(first) = candidates.first() else {
        return String::new();
    };
    let common_length = candidates
        .iter()
        .skip(1)
        .fold(first.chars().count(), |limit, item| {
            first
                .chars()
                .zip(item.chars())
                .take(limit)
                .take_while(|(left, right)| left == right)
                .count()
        });
    first.chars().take(common_length).collect()
}

fn replace_editor_range(buffer: &gtk::TextBuffer, start: usize, end: usize, replacement: &str) {
    let mut start_iter = buffer.iter_at_offset(start as i32);
    let mut end_iter = buffer.iter_at_offset(end as i32);
    buffer.delete(&mut start_iter, &mut end_iter);
    buffer.insert(&mut start_iter, replacement);
    buffer.place_cursor(&start_iter);
}

fn insert_terminal_segment(
    buffer: &gtk::TextBuffer,
    text: &str,
    colorize: bool,
    style: &AnsiStyle,
    tags: &mut HashMap<AnsiStyle, gtk::TextTag>,
) {
    if text.is_empty() {
        return;
    }
    let start_offset = buffer.char_count();
    let mut insertion_point = buffer.end_iter();
    buffer.insert(&mut insertion_point, text);
    if !colorize || *style == AnsiStyle::default() {
        return;
    }

    let tag = tags.entry(style.clone()).or_insert_with(|| {
        let mut builder = gtk::TextTag::builder();
        if let Some(color) = &style.foreground {
            builder = builder.foreground(color);
        }
        if let Some(color) = &style.background {
            builder = builder.background(color);
        }
        if style.bold {
            builder = builder.weight(700);
        }
        if style.underline {
            builder = builder.underline(gtk::pango::Underline::Single);
        }
        let tag = builder.build();
        buffer.tag_table().add(&tag);
        tag
    });
    let start = buffer.iter_at_offset(start_offset);
    let end = buffer.iter_at_offset(start_offset + text.chars().count() as i32);
    buffer.apply_tag(tag, &start, &end);
}

fn apply_ansi_parameters(style: &mut AnsiStyle, parameters: &str, palette: OutputPalette) {
    let values: Vec<u16> = if parameters.is_empty() {
        vec![0]
    } else {
        parameters
            .split(';')
            .map(|value| value.parse::<u16>().unwrap_or(0))
            .collect()
    };
    let mut index = 0;
    while index < values.len() {
        match values[index] {
            0 => *style = AnsiStyle::default(),
            1 => style.bold = true,
            22 => style.bold = false,
            4 => style.underline = true,
            24 => style.underline = false,
            30..=37 => {
                style.foreground = Some(ansi_basic_color(values[index] - 30, false, palette))
            }
            39 => style.foreground = None,
            40..=47 => {
                style.background = Some(ansi_basic_color(values[index] - 40, false, palette))
            }
            49 => style.background = None,
            90..=97 => style.foreground = Some(ansi_basic_color(values[index] - 90, true, palette)),
            100..=107 => {
                style.background = Some(ansi_basic_color(values[index] - 100, true, palette))
            }
            38 | 48 if values.get(index + 1) == Some(&5) => {
                if let Some(color) = values.get(index + 2).copied() {
                    let color = ansi_256_color(color);
                    if values[index] == 38 {
                        style.foreground = Some(color);
                    } else {
                        style.background = Some(color);
                    }
                    index += 2;
                }
            }
            38 | 48 if values.get(index + 1) == Some(&2) && index + 4 < values.len() => {
                let color = format!(
                    "#{:02x}{:02x}{:02x}",
                    values[index + 2].min(255),
                    values[index + 3].min(255),
                    values[index + 4].min(255)
                );
                if values[index] == 38 {
                    style.foreground = Some(color);
                } else {
                    style.background = Some(color);
                }
                index += 4;
            }
            _ => {}
        }
        index += 1;
    }
}

fn ansi_basic_color(index: u16, bright: bool, palette: OutputPalette) -> String {
    const STANDARD: [&str; 16] = [
        "#666666", "#cd3131", "#0dbc79", "#e5e510", "#2472c8", "#bc3fbc", "#11a8cd", "#e5e5e5",
        "#808080", "#f14c4c", "#23d18b", "#f5f543", "#3b8eea", "#d670d6", "#29b8db", "#ffffff",
    ];
    const HIGH_CONTRAST: [&str; 16] = [
        "#8b949e", "#ff7b72", "#3fb950", "#d29922", "#58a6ff", "#bc8cff", "#39c5cf", "#f0f6fc",
        "#6e7681", "#ffa198", "#56d364", "#e3b341", "#79c0ff", "#d2a8ff", "#56d4dd", "#ffffff",
    ];
    let color_index = usize::from(index.min(7)) + if bright { 8 } else { 0 };
    match palette {
        OutputPalette::Standard => STANDARD[color_index],
        OutputPalette::HighContrast => HIGH_CONTRAST[color_index],
    }
    .to_owned()
}

fn ansi_256_color(index: u16) -> String {
    const BASE: [&str; 16] = [
        "#666666", "#cd3131", "#0dbc79", "#e5e510", "#2472c8", "#bc3fbc", "#11a8cd", "#e5e5e5",
        "#808080", "#f14c4c", "#23d18b", "#f5f543", "#3b8eea", "#d670d6", "#29b8db", "#ffffff",
    ];
    match index {
        0..=15 => BASE[index as usize].to_owned(),
        16..=231 => {
            let value = index - 16;
            let levels = [0, 95, 135, 175, 215, 255];
            format!(
                "#{:02x}{:02x}{:02x}",
                levels[(value / 36) as usize],
                levels[((value / 6) % 6) as usize],
                levels[(value % 6) as usize]
            )
        }
        _ => {
            let gray = 8 + (index.min(255) - 232) * 10;
            format!("#{gray:02x}{gray:02x}{gray:02x}")
        }
    }
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
         .shell-output {{ background: alpha(@theme_fg_color, 0.035); }}\n\
         .shell-prompt {{ color: @accent_color; font-family: monospace; font-weight: bold; }}\n"
    ));
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completion_uses_longest_shared_prefix() {
        assert_eq!(
            common_prefix(&["git ".to_owned(), "gitk ".to_owned()]),
            "git"
        );
        assert_eq!(common_prefix(&[]), "");
    }

    #[test]
    fn completed_paths_escape_spaces_for_the_shell() {
        assert_eq!(
            format_shell_word("My Documents/file name", None, false),
            "My\\ Documents/file\\ name "
        );
    }

    #[test]
    fn token_scanner_keeps_quoted_and_escaped_spaces_together() {
        let quoted: Vec<char> = "echo \"My files/na".chars().collect();
        let cursor = quoted.len();
        let start = shell_word_start(&quoted, 0, cursor);
        assert_eq!(quoted[start..].iter().collect::<String>(), "\"My files/na");
        assert_eq!(shell_word_end(&quoted, start, cursor), cursor);
        assert_eq!(unescape_shell_word("\"My files/na"), "My files/na");

        let escaped: Vec<char> = "cd My\\ files/na".chars().collect();
        let cursor = escaped.len();
        let start = shell_word_start(&escaped, 0, cursor);
        assert_eq!(escaped[start..].iter().collect::<String>(), "My\\ files/na");
        assert_eq!(unescape_shell_word("My\\ files/na"), "My files/na");
    }

    #[test]
    fn completion_detects_command_positions_after_shell_operators() {
        assert!(command_position(&[]));
        assert!(command_position(&"sudo ".chars().collect::<Vec<_>>()));
        assert!(command_position(
            &"echo done && ".chars().collect::<Vec<_>>()
        ));
        assert!(!command_position(&"echo ".chars().collect::<Vec<_>>()));
    }

    #[test]
    fn installed_commands_and_shell_builtins_are_recognized() {
        let names = installed_command_names();
        assert!(shell_builtin("cd"));
        assert!(is_executable_command("ls", &names), "{names:?}");
        assert!(!is_executable_command(
            "not-a-real-editshell-command",
            &names
        ));
        assert!(
            command_candidates("ls")
                .iter()
                .any(|candidate| candidate == "ls ")
        );
    }
}
