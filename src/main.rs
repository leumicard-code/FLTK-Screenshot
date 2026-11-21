mod capture;
mod editor;
use std::cell::{Cell, RefCell};
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use gtk4::gdk;
use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, Box as GtkBox, Button, DrawingArea, DropDown,
    EventControllerKey, FileDialog, GestureDrag, HeaderBar, Label, Orientation, ToggleButton,
};

use capture::CaptureMode;
use editor::{Annotation, EditorState, Tool};

const APP_ID: &str = "com.quickshot.App";

fn main() -> glib::ExitCode {
    let app = Application::builder()
    .application_id(APP_ID)
    .flags(gio::ApplicationFlags::NON_UNIQUE)
    .build();
    app.connect_activate(build_ui);
    app.run()
}

fn build_ui(app: &Application) {
    let last_capture: Rc<RefCell<Option<PathBuf>>> = Rc::new(RefCell::new(None));
    let editor_state: Rc<RefCell<Option<EditorState>>> = Rc::new(RefCell::new(None));
    let active_tool = Rc::new(Cell::new(Tool::Arrow));

    // 1. כפתורי צילום + השהיה (צד שמאל של ה-HeaderBar)
    let btn_full = Button::with_label("מסך מלא");
    let btn_area = Button::with_label("בחר אזור");
    let delay_dropdown = DropDown::from_strings(&["0s", "3s", "5s"]);
    delay_dropdown.set_tooltip_text(Some("השהיה לפני צילום"));

    // 2. סרגל כלי העריכה במרכז ה-HeaderBar
    let btn_arrow = ToggleButton::with_label("חץ");
    let btn_rect = ToggleButton::with_label("מלבן");
    let btn_blur = ToggleButton::with_label("טשטוש");
    btn_rect.set_group(Some(&btn_arrow));
    btn_blur.set_group(Some(&btn_arrow));
    let btn_crop = ToggleButton::with_label("חיתוך");
    btn_crop.set_group(Some(&btn_arrow));
    btn_arrow.set_active(true);

    let btn_undo = Button::with_label("בטל");
    btn_undo.set_tooltip_text(Some("בטל פעולה אחרונה (Ctrl+Z)"));
    btn_undo.set_sensitive(false);

    let tools_box = GtkBox::new(Orientation::Horizontal, 0);
    tools_box.add_css_class("linked");
    tools_box.append(&btn_arrow);
    tools_box.append(&btn_rect);
    tools_box.append(&btn_blur);
    tools_box.append(&btn_crop);

    




    let center_box = GtkBox::new(Orientation::Horizontal, 8);
    center_box.append(&tools_box);
    center_box.append(&btn_undo);

    // 3. כפתורי פעולה על התמונה (צד ימין של ה-HeaderBar)
    let btn_copy = Button::with_label("העתק");
    btn_copy.set_tooltip_text(Some("העתק ל-Clipboard (Ctrl+C)"));
    btn_copy.set_sensitive(false);

    let btn_save = Button::with_label("שמור...");
    btn_save.set_tooltip_text(Some("שמור בשם (Ctrl+S)"));
    btn_save.set_sensitive(false);

    let header = HeaderBar::new();
    header.pack_start(&btn_full);
    header.pack_start(&btn_area);
    header.pack_start(&delay_dropdown);
    header.set_title_widget(Some(&center_box));
    header.pack_end(&btn_save);
    header.pack_end(&btn_copy);

    // 4. קנבס העריכה והתצוגה המקדימה (Cairo)
    let canvas = DrawingArea::new();
    canvas.set_hexpand(true);
    canvas.set_vexpand(true);
    {
        let editor_state = editor_state.clone();
        canvas.set_draw_func(move |_, cr, width, height| {
                // רקע כהה עדין סביב התמונה
                cr.set_source_rgb(0.12, 0.12, 0.14);
                let _ = cr.paint();

                if let Some(ed) = editor_state.borrow().as_ref() {
                    ed.draw_on_widget(cr, width as f64, height as f64);
                }
        });
    }

    // 5. שורת סטטוס דקה למטה
    let status_label = Label::new(Some(
            "מוכן לצילום  •  גרור לציור  •  Ctrl+Z בטל  •  Ctrl+C העתק  •  Ctrl+S שמור",
    ));
    status_label.set_xalign(0.0);
    status_label.set_margin_start(10);
    status_label.set_margin_end(10);
    status_label.set_margin_bottom(6);

    let root = GtkBox::new(Orientation::Vertical, 6);
    root.append(&canvas);
    root.append(&status_label);

    let window = ApplicationWindow::builder()
    .application(app)
    .title("QuickShot")
    .default_width(920)
    .default_height(600)
    .child(&root)
    .build();
    window.set_titlebar(Some(&header));

    // חיבור בחירת הכלים (חץ / מלבן / טשטוש)
    connect_tool_button(&btn_arrow, Tool::Arrow, &active_tool, &editor_state);
    connect_tool_button(&btn_rect, Tool::Rect, &active_tool, &editor_state);
    connect_tool_button(&btn_blur, Tool::Blur, &active_tool, &editor_state);
    connect_tool_button(&btn_crop, Tool::Crop, &active_tool, &editor_state);

    // חיבור גרירת עכבר לציור על הקנבס
    {
        let drag = GestureDrag::new();
        let drag_start = Rc::new(Cell::new((0.0_f64, 0.0_f64)));

        {
            let editor_state = editor_state.clone();
            let canvas_ref = canvas.clone();
            let drag_start = drag_start.clone();

            drag.connect_drag_begin(move |_, start_x, start_y| {
                    drag_start.set((start_x, start_y));
                    let mut borrow = editor_state.borrow_mut();
                    if let Some(ed) = borrow.as_mut() {
                        let (ix, iy) = ed.widget_to_image(
                            start_x,
                            start_y,
                            canvas_ref.width() as f64,
                            canvas_ref.height() as f64,
                        );
                        ed.draft = Some(Annotation {
                                tool: ed.current_tool,
                                x1: ix,
                                y1: iy,
                                x2: ix,
                                y2: iy,
                        });
                        canvas_ref.queue_draw();
                    }
            });
        }

        {
            let editor_state = editor_state.clone();
            let canvas_ref = canvas.clone();
            let drag_start = drag_start.clone();

            drag.connect_drag_update(move |_, offset_x, offset_y| {
                    let (sx, sy) = drag_start.get();
                    let mut borrow = editor_state.borrow_mut();
                    if let Some(ed) = borrow.as_mut() {
                        let (ix, iy) = ed.widget_to_image(
                            sx + offset_x,
                            sy + offset_y,
                            canvas_ref.width() as f64,
                            canvas_ref.height() as f64,
                        );
                        if let Some(draft) = ed.draft.as_mut() {
                            draft.x2 = ix;
                            draft.y2 = iy;
                        }
                        canvas_ref.queue_draw();
                    }
            });
        }

        {
            let editor_state = editor_state.clone();
            let canvas_ref = canvas.clone();
            let btn_undo = btn_undo.clone();

            drag.connect_drag_end(move |_, _, _| {
                    let mut borrow = editor_state.borrow_mut();
                    if let Some(ed) = borrow.as_mut() {
                        if let Some(draft) = ed.draft.take() {
                            if draft.tool == Tool::Crop {
                                let _ = ed.apply_crop(draft.x1, draft.y1, draft.x2, draft.y2);
                            } else if (draft.x2 - draft.x1).hypot(draft.y2 - draft.y1) >= 3.0 {
                                ed.annotations.push(draft);
                            }
                        }
                        btn_undo.set_sensitive(!ed.annotations.is_empty());
                        canvas_ref.queue_draw();
                    }
            });
        }

        canvas.add_controller(drag);
    }

    // חיבור כפתור "בטל" (Undo)
    {
        let editor_state = editor_state.clone();
        let canvas_ref = canvas.clone();
        let btn_undo_ref = btn_undo.clone();

        btn_undo.connect_clicked(move |_| {
                let mut borrow = editor_state.borrow_mut();
                if let Some(ed) = borrow.as_mut() {
                    if ed.undo() {
                        canvas_ref.queue_draw();
                    }
                    btn_undo_ref.set_sensitive(!ed.annotations.is_empty());
                }
        });
    }

    // חיבור כפתורי הצילום
    connect_capture_button(
        &btn_full,
        CaptureMode::FullScreen,
        &delay_dropdown,
        &window,
        &canvas,
        &status_label,
        &btn_copy,
        &btn_save,
        &btn_undo,
        &last_capture,
        &editor_state,
        &active_tool,
    );
    connect_capture_button(
        &btn_area,
        CaptureMode::Interactive,
        &delay_dropdown,
        &window,
        &canvas,
        &status_label,
        &btn_copy,
        &btn_save,
        &btn_undo,
        &last_capture,
        &editor_state,
        &active_tool,
    );

    // חיבור כפתור "העתק" (מייצא את התמונה כולל העריכות ל-Clipboard)
    {
        let editor_state = editor_state.clone();
        let status_label = status_label.clone();
        let window_ref = window.clone();

        btn_copy.connect_clicked(move |_| {
                let borrow = editor_state.borrow();
                let Some(ed) = borrow.as_ref() else {
                    return;
                };

                match ed.export_texture() {
                    Ok(texture) => {
                        let clipboard = WidgetExt::display(&window_ref).clipboard();
                        clipboard.set_texture(&texture);
                        status_label.set_text("התמונה הערוכה הועתקה ל-Clipboard!");
                    }
                    Err(err) => {
                        status_label.set_text(&format!("שגיאה בייצוא ל-Clipboard: {err}"));
                    }
                }
        });
    }

    // חיבור כפתור "שמור..." (שומר את התמונה כולל העריכות כ-PNG)
    {
        let editor_state = editor_state.clone();
        let status_label = status_label.clone();
        let window_ref = window.clone();

        btn_save.connect_clicked(move |_| {
                let texture = {
                    let borrow = editor_state.borrow();
                    let Some(ed) = borrow.as_ref() else {
                        return;
                    };
                    match ed.export_texture() {
                        Ok(tex) => tex,
                        Err(err) => {
                            status_label.set_text(&format!("שגיאה בייצוא התמונה: {err}"));
                            return;
                        }
                    }
                };

                let dialog = FileDialog::builder()
                .title("שמור צילום מסך")
                .initial_name("screenshot.png")
                .build();

                let status_label = status_label.clone();
                dialog.save(Some(&window_ref), gio::Cancellable::NONE, move |res| {
                        if let Ok(file) = res {
                            if let Some(dest_path) = file.path() {
                                match texture.save_to_png(&dest_path) {
                                    Ok(_) => status_label
                                    .set_text(&format!("נשמר בהצלחה: {}", dest_path.display())),
                                    Err(err) => status_label.set_text(&format!("שגיאה בשמירה: {err}")),
                                }
                            }
                        }
                });
        });
    }

    // קיצורי מקלדת: Ctrl+C, Ctrl+S, Ctrl+Z, Esc
    {
        let key_controller = EventControllerKey::new();
        let btn_copy = btn_copy.clone();
        let btn_save = btn_save.clone();
        let btn_undo = btn_undo.clone();
        let window_ref = window.clone();

        key_controller.connect_key_pressed(move |_, key, _, modifier| {
                let is_ctrl = modifier.contains(gdk::ModifierType::CONTROL_MASK);

                if is_ctrl && matches!(key, gdk::Key::c | gdk::Key::C) {
                    if btn_copy.is_sensitive() {
                        btn_copy.emit_clicked();
                    }
                    return glib::Propagation::Stop;
                }

                if is_ctrl && matches!(key, gdk::Key::s | gdk::Key::S) {
                    if btn_save.is_sensitive() {
                        btn_save.emit_clicked();
                    }
                    return glib::Propagation::Stop;
                }

                if is_ctrl && matches!(key, gdk::Key::z | gdk::Key::Z) {
                    if btn_undo.is_sensitive() {
                        btn_undo.emit_clicked();
                    }
                    return glib::Propagation::Stop;
                }

                if key == gdk::Key::Escape {
                    window_ref.close();
                    return glib::Propagation::Stop;
                }

                glib::Propagation::Proceed
        });
        window.add_controller(key_controller);
    }

    // ניקוי קובץ זמני בסגירת החלון
    {
        let last_capture = last_capture.clone();
        window.connect_close_request(move |_| {
                if let Some(path) = last_capture.borrow_mut().take() {
                    cleanup_temp_file(&path);
                }
                glib::Propagation::Proceed
        });
    }

    window.present();
}

fn connect_tool_button(
    button: &ToggleButton,
    tool: Tool,
    active_tool: &Rc<Cell<Tool>>,
    editor_state: &Rc<RefCell<Option<EditorState>>>,
) {
    let active_tool = active_tool.clone();
    let editor_state = editor_state.clone();

    button.connect_toggled(move |btn| {
            if btn.is_active() {
                active_tool.set(tool);
                if let Some(ed) = editor_state.borrow_mut().as_mut() {
                    ed.current_tool = tool;
                }
            }
    });
}

fn cleanup_temp_file(path: &Path) {
    let temp_dir = std::env::temp_dir();
    if path.starts_with(&temp_dir) {
        let _ = fs::remove_file(path);
    }
}

fn selected_delay_secs(dropdown: &DropDown) -> u64 {
    match dropdown.selected() {
        1 => 3,
        2 => 5,
        _ => 0,
    }
}

#[allow(clippy::too_many_arguments)]
fn connect_capture_button(
    button: &Button,
    mode: CaptureMode,
    delay_dropdown: &DropDown,
    window: &ApplicationWindow,
    canvas: &DrawingArea,
    status_label: &Label,
    btn_copy: &Button,
    btn_save: &Button,
    btn_undo: &Button,
    last_capture: &Rc<RefCell<Option<PathBuf>>>,
    editor_state: &Rc<RefCell<Option<EditorState>>>,
    active_tool: &Rc<Cell<Tool>>,
) {
    let delay_dropdown = delay_dropdown.clone();
    let window = window.clone();
    let canvas = canvas.clone();
    let status_label = status_label.clone();
    let btn_copy = btn_copy.clone();
    let btn_save = btn_save.clone();
    let btn_undo = btn_undo.clone();
    let last_capture = last_capture.clone();
    let editor_state = editor_state.clone();
    let active_tool = active_tool.clone();

    button.connect_clicked(move |_| {
            let delay_secs = selected_delay_secs(&delay_dropdown);
            let window = window.clone();
            let canvas = canvas.clone();
            let status_label = status_label.clone();
            let btn_copy = btn_copy.clone();
            let btn_save = btn_save.clone();
            let btn_undo = btn_undo.clone();
            let last_capture = last_capture.clone();
            let editor_state = editor_state.clone();
            let active_tool = active_tool.clone();

            glib::spawn_future_local(async move {
                    let _hold = window.application().map(|app| app.hold());
                    window.set_visible(false);

                    let wait = if delay_secs > 0 {
                        Duration::from_secs(delay_secs)
                    } else {
                        Duration::from_millis(150)
                    };
                    glib::timeout_future(wait).await;

                    match capture::take_screenshot(mode).await {
                        Ok(path) => match EditorState::from_file(&path) {
                            Ok(mut ed) => {
                                ed.current_tool = active_tool.get();
                                *editor_state.borrow_mut() = Some(ed);

                                if let Some(old_path) = last_capture.borrow_mut().replace(path.clone()) {
                                    cleanup_temp_file(&old_path);
                                }

                                btn_copy.set_sensitive(true);
                                btn_save.set_sensitive(true);
                                btn_undo.set_sensitive(false);
                                canvas.queue_draw();
                                status_label.set_text(&format!("צולם בהצלחה: {}", path.display()));
                            }
                            Err(err) => {
                                status_label.set_text(&format!("שגיאה בטעינת התמונה לעורך: {err}"));
                            }
                        },
                        Err(err) => {
                            status_label.set_text(&format!("בוטל / שגיאה: {err}"));
                        }
                    }

                    window.present();
            });
    });
}