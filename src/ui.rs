use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    rc::Rc,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use gpui_kit::assets::IconName;
use gpui_kit::component::{
    button::{Button, ButtonVariants as _},
    input::{Input, InputState},
    progress::Progress,
    scroll::Scrollbar,
    switch::Switch,
    tab::{Tab, TabBar},
    v_virtual_list, Disableable as _, Icon, Sizable as _, StyledExt as _, Theme, ThemeMode,
    VirtualListScrollHandle,
};
use gpui_kit::prelude::*;
use gpui_kit::{
    div, linear_color_stop, linear_gradient, px, rgb, rgba, size, AnyElement, App, Context, Entity,
    Render, TitlebarOptions, Window, WindowBounds, WindowOptions,
};

use crate::core;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Screen {
    Home,
    Analysis,
    Results,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SizeUnit {
    B,
    Kb,
    Mb,
    Gb,
}

impl SizeUnit {
    const ALL: [SizeUnit; 4] = [SizeUnit::B, SizeUnit::Kb, SizeUnit::Mb, SizeUnit::Gb];

    fn label(self) -> &'static str {
        match self {
            SizeUnit::B => "B",
            SizeUnit::Kb => "KB",
            SizeUnit::Mb => "MB",
            SizeUnit::Gb => "GB",
        }
    }

    fn multiplier(self) -> u64 {
        match self {
            SizeUnit::B => 1,
            SizeUnit::Kb => 1024,
            SizeUnit::Mb => 1024 * 1024,
            SizeUnit::Gb => 1024 * 1024 * 1024,
        }
    }

    fn index(self) -> usize {
        match self {
            SizeUnit::B => 0,
            SizeUnit::Kb => 1,
            SizeUnit::Mb => 2,
            SizeUnit::Gb => 3,
        }
    }

    fn from_index(index: usize) -> Self {
        match index {
            0 => SizeUnit::B,
            1 => SizeUnit::Kb,
            2 => SizeUnit::Mb,
            _ => SizeUnit::Gb,
        }
    }
}

#[derive(Clone, Debug)]
enum Verification {
    Idle,
    Checking,
    Done { hash: String, removed: usize },
    NoDuplicates,
    Error(String),
}

struct Group {
    hash: String,
    size: String,
    size_bytes: u64,
    sampled: bool,
    files: Vec<String>,
    verification: Verification,
}

struct Results {
    root: PathBuf,
    quick_mode: bool,
    groups: Vec<Group>,
    skipped: Vec<String>,
    pending_delete: Option<(usize, String)>,
    scroll: VirtualListScrollHandle,
}

impl Results {
    fn new(root: PathBuf, report: core::Report, quick_mode: bool) -> Self {
        Self {
            root,
            quick_mode,
            groups: report
                .pairs
                .into_iter()
                .map(|record| Group {
                    hash: record.hash,
                    size: record.size,
                    size_bytes: record.size_bytes,
                    sampled: record.sampled,
                    files: record.files,
                    verification: Verification::Idle,
                })
                .collect(),
            skipped: report.skipped,
            pending_delete: None,
            scroll: VirtualListScrollHandle::new(),
        }
    }

    fn apply_verification(
        &mut self,
        index: usize,
        result: Result<core::GroupVerification, String>,
    ) {
        let Some(group) = self.groups.get_mut(index) else {
            return;
        };

        let report = match result {
            Ok(report) => report,
            Err(error) => {
                group.verification = Verification::Error(error);
                return;
            }
        };

        if report.files.iter().any(|file| file.hash.is_none()) {
            let message = report
                .files
                .iter()
                .find(|file| file.hash.is_none())
                .and_then(|file| file.error.clone())
                .unwrap_or_else(|| {
                    "Could not read every file in this group, so nothing was removed.".to_string()
                });
            group.verification = Verification::Error(message);
            return;
        }

        // Keep the largest set of files that share a full hash. Files outside
        // that set only matched on the 4 KB sample and are not duplicates.
        let mut by_hash: HashMap<&str, Vec<&str>> = HashMap::new();
        for file in &report.files {
            if let Some(hash) = file.hash.as_deref() {
                by_hash.entry(hash).or_default().push(&file.path);
            }
        }

        let Some((best_hash, best_paths)) =
            by_hash.into_iter().max_by_key(|(_, paths)| paths.len())
        else {
            group.verification = Verification::Error("No file hashes were returned.".into());
            return;
        };

        if best_paths.len() < 2 {
            group.files.clear();
            group.verification = Verification::NoDuplicates;
            return;
        }

        let keep: HashSet<&str> = best_paths.into_iter().collect();
        let before = group.files.len();
        group.files.retain(|path| keep.contains(path.as_str()));
        group.verification = Verification::Done {
            hash: best_hash.to_string(),
            removed: before - group.files.len(),
        };
    }
}

enum BgEvent {
    Progress(core::AnalysisEvent),
    FolderPicked(Option<(PathBuf, core::Config)>),
    AnalysisDone(Result<core::Report, String>),
    Verified {
        index: usize,
        result: Result<core::GroupVerification, String>,
    },
}

#[derive(Clone)]
struct EventQueue(Arc<Mutex<Vec<BgEvent>>>);

impl EventQueue {
    fn new() -> Self {
        Self(Arc::new(Mutex::new(Vec::new())))
    }

    fn push(&self, event: BgEvent) {
        if let Ok(mut events) = self.0.lock() {
            events.push(event);
        }
    }

    fn drain(&self) -> Vec<BgEvent> {
        self.0
            .lock()
            .map(|mut events| std::mem::take(&mut *events))
            .unwrap_or_default()
    }
}

struct AnalysisRun {
    root: PathBuf,
    cancel: Arc<AtomicBool>,
    cancelling: bool,
    stage: usize,
    quick: Option<(usize, usize)>,
    full: Option<(usize, usize)>,
}

impl AnalysisRun {
    fn new(root: PathBuf, cancel: Arc<AtomicBool>) -> Self {
        Self {
            root,
            cancel,
            cancelling: false,
            stage: 0,
            quick: None,
            full: None,
        }
    }

    fn apply(&mut self, event: core::AnalysisEvent) {
        match event {
            core::AnalysisEvent::IngestionDone => self.stage = self.stage.max(1),
            core::AnalysisEvent::QuickProgress { done, total } => {
                let done = self
                    .quick
                    .map(|(previous, _)| previous.max(done))
                    .unwrap_or(done);
                self.quick = Some((done, total));
            }
            core::AnalysisEvent::QuickDone => {
                self.stage = self.stage.max(2);
                if let Some((_, total)) = self.quick {
                    self.quick = Some((total, total));
                }
            }
            core::AnalysisEvent::FullProgress { done, total } => {
                let done = self
                    .full
                    .map(|(previous, _)| previous.max(done))
                    .unwrap_or(done);
                self.full = Some((done, total));
            }
            core::AnalysisEvent::FullDone => {
                self.stage = 3;
                if let Some((_, total)) = self.full {
                    self.full = Some((total, total));
                }
            }
        }
    }
}

struct DedupperApp {
    include: Entity<InputState>,
    exclude: Entity<InputState>,
    min_size: Entity<InputState>,
    max_size: Entity<InputState>,
    min_unit: SizeUnit,
    max_unit: SizeUnit,
    quick_mode: bool,
    picking: bool,
    screen: Screen,
    run: Option<AnalysisRun>,
    results: Option<Results>,
    error: Option<String>,
    events: EventQueue,
}

impl DedupperApp {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let include = cx.new(|cx| InputState::new(window, cx).placeholder("**/*.txt, **/*.mp4"));
        let exclude =
            cx.new(|cx| InputState::new(window, cx).placeholder("**/node_modules, **/*.tmp"));
        let min_size = cx.new(|cx| InputState::new(window, cx).default_value("1"));
        let max_size = cx.new(|cx| InputState::new(window, cx).default_value("10"));

        Self {
            include,
            exclude,
            min_size,
            max_size,
            min_unit: SizeUnit::Kb,
            max_unit: SizeUnit::Gb,
            quick_mode: false,
            picking: false,
            screen: Screen::Home,
            run: None,
            results: None,
            error: None,
            events: EventQueue::new(),
        }
    }

    fn start_event_pump(&self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| loop {
            cx.background_executor()
                .timer(Duration::from_millis(40))
                .await;
            let alive = this.update(cx, |app, cx| app.drain_events(cx)).is_ok();
            if !alive {
                break;
            }
        })
        .detach();
    }

    fn drain_events(&mut self, cx: &mut Context<Self>) {
        let events = self.events.drain();
        if events.is_empty() {
            return;
        }

        for event in events {
            match event {
                BgEvent::Progress(event) => {
                    if let Some(run) = self.run.as_mut() {
                        run.apply(event);
                    }
                }
                BgEvent::FolderPicked(selection) => {
                    self.picking = false;
                    if let Some((path, config)) = selection {
                        self.start_analysis(path, config, cx);
                    }
                }
                BgEvent::AnalysisDone(result) => {
                    let run = self.run.take();
                    match result {
                        Ok(report) if report.cancelled => {
                            self.results = None;
                            self.screen = Screen::Home;
                        }
                        Ok(report) => {
                            if let Some(run) = run {
                                self.results =
                                    Some(Results::new(run.root, report, self.quick_mode));
                                self.screen = Screen::Results;
                            }
                        }
                        Err(error) => {
                            self.results = None;
                            self.error = Some(error);
                            self.screen = Screen::Home;
                        }
                    }
                }
                BgEvent::Verified { index, result } => {
                    if let Some(results) = self.results.as_mut() {
                        results.apply_verification(index, result);
                    }
                }
            }
        }

        cx.notify();
    }

    fn build_config(&self, cx: &App) -> Result<core::Config, String> {
        let include = self.include.read(cx).value().trim().to_string();
        let exclude = self.exclude.read(cx).value().trim().to_string();
        let min_value = parse_size_value(&self.min_size.read(cx).value())
            .ok_or_else(|| "Enter a number for the minimum size.".to_string())?;
        let max_value = parse_size_value(&self.max_size.read(cx).value())
            .ok_or_else(|| "Enter a number for the maximum size.".to_string())?;
        let min = (min_value * self.min_unit.multiplier() as f64) as u64;
        let max = (max_value * self.max_unit.multiplier() as f64) as u64;
        if max < min {
            return Err("Maximum size must be greater than or equal to the minimum size.".into());
        }

        Ok(core::Config {
            include,
            exclude,
            size_extend: (min, max),
            quick_mode: self.quick_mode,
        })
    }

    fn choose_directory(&mut self, cx: &mut Context<Self>) {
        let config = match self.build_config(cx) {
            Ok(config) => config,
            Err(error) => {
                self.error = Some(error);
                cx.notify();
                return;
            }
        };

        // The native dialog runs its own modal message loop. Opening it from
        // a GPUI event callback would let that loop re-enter GPUI while the
        // app is borrowed, so run it on a background thread and deliver the
        // result through the event queue.
        self.picking = true;
        self.error = None;
        let events = self.events.clone();
        cx.spawn(async move |_, cx| {
            let picked = cx
                .background_executor()
                .spawn(async move { rfd::FileDialog::new().pick_folder() })
                .await;
            events.push(BgEvent::FolderPicked(picked.map(|path| (path, config))));
        })
        .detach();

        cx.notify();
    }

    fn start_analysis(&mut self, root: PathBuf, config: core::Config, cx: &mut Context<Self>) {
        self.error = None;
        self.results = None;
        self.screen = Screen::Analysis;

        let cancel = Arc::new(AtomicBool::new(false));
        self.run = Some(AnalysisRun::new(root.clone(), cancel.clone()));
        let events = self.events.clone();
        let analysis_events = events.clone();

        cx.spawn(async move |_, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    core::analyze(&root, &config, &cancel, move |event| {
                        analysis_events.push(BgEvent::Progress(event));
                    })
                })
                .await;
            events.push(BgEvent::AnalysisDone(result));
        })
        .detach();

        cx.notify();
    }

    fn cancel_analysis(&mut self, cx: &mut Context<Self>) {
        if let Some(run) = self.run.as_mut() {
            run.cancel.store(true, Ordering::Relaxed);
            run.cancelling = true;
        }
        cx.notify();
    }

    fn go_back(&mut self, cx: &mut Context<Self>) {
        self.results = None;
        self.error = None;
        self.screen = Screen::Home;
        cx.notify();
    }

    fn verify_group(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(results) = self.results.as_mut() else {
            return;
        };
        let Some(group) = results.groups.get_mut(index) else {
            return;
        };
        if matches!(&group.verification, Verification::Checking) {
            return;
        }

        group.verification = Verification::Checking;
        let root = results.root.clone();
        let files = group.files.clone();
        let events = self.events.clone();

        cx.spawn(async move |_, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { core::full_hash_group(&root, &files) })
                .await;
            events.push(BgEvent::Verified { index, result });
        })
        .detach();

        cx.notify();
    }

    fn request_delete(&mut self, index: usize, file: String, cx: &mut Context<Self>) {
        if let Some(results) = self.results.as_mut() {
            results.pending_delete = Some((index, file));
        }
        cx.notify();
    }

    fn cancel_delete(&mut self, cx: &mut Context<Self>) {
        if let Some(results) = self.results.as_mut() {
            results.pending_delete = None;
        }
        cx.notify();
    }

    fn confirm_delete(&mut self, cx: &mut Context<Self>) {
        let Some(results) = self.results.as_mut() else {
            return;
        };
        let Some((index, file)) = results.pending_delete.clone() else {
            return;
        };

        match core::remove_file(&results.root, &file) {
            Ok(()) => {
                if let Some(group) = results.groups.get_mut(index) {
                    group.files.retain(|path| path != &file);
                    group.verification = Verification::Idle;
                }
                results.pending_delete = None;
            }
            Err(error) => {
                results.pending_delete = None;
                self.error = Some(format!("Could not delete \"{file}\": {error}"));
            }
        }

        cx.notify();
    }

    fn reveal(&mut self, file: String, cx: &mut Context<Self>) {
        let Some(results) = self.results.as_ref() else {
            return;
        };
        if let Err(error) = core::reveal_file_in_explorer(&results.root, &file) {
            self.error = Some(format!("Could not reveal the file: {error}"));
            cx.notify();
        }
    }

    fn render_home(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let border = palette_border();
        let muted = palette_muted();
        let danger = palette_danger();

        let app = cx.entity();
        let min_unit_group = TabBar::new("min-unit")
            .xsmall()
            .pill()
            .last_empty_space(div().w(px(0.)))
            .selected_index(self.min_unit.index())
            .on_click({
                let app = app.clone();
                move |index, _, cx| {
                    let unit = SizeUnit::from_index(*index);
                    app.update(cx, |this, cx| {
                        this.min_unit = unit;
                        cx.notify();
                    });
                }
            })
            .children(SizeUnit::ALL.map(|unit| Tab::new().label(unit.label())));
        let max_unit_group = TabBar::new("max-unit")
            .xsmall()
            .pill()
            .last_empty_space(div().w(px(0.)))
            .selected_index(self.max_unit.index())
            .on_click({
                let app = app.clone();
                move |index, _, cx| {
                    let unit = SizeUnit::from_index(*index);
                    app.update(cx, |this, cx| {
                        this.max_unit = unit;
                        cx.notify();
                    });
                }
            })
            .children(SizeUnit::ALL.map(|unit| Tab::new().label(unit.label())));

        let mut content = div()
            .w_full()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .w(px(44.))
                            .h(px(44.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(13.))
                            .border_1()
                            .border_color(border)
                            .bg(palette_accent_soft())
                            .child(
                                Icon::new(IconName::FolderSearch)
                                    .size(px(22.))
                                    .text_color(palette_accent_deep()),
                            ),
                    )
                    .child(div().text_2xl().font_bold().child("Find duplicate files"))
                    .child(div().text_sm().text_color(muted).child(
                        "Scan a folder for redundant copies. Everything stays on this device.",
                    )),
            )
            .child(self.render_mode_toggle(cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .pt_4()
                    .border_t_1()
                    .border_color(border)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_xs()
                                    .font_bold()
                                    .text_color(muted)
                                    .child("FILTERS"),
                            )
                            .child(div().text_xs().text_color(muted).child("optional")),
                    )
                    .child(input_field("Include", &self.include))
                    .child(input_field("Exclude", &self.exclude))
                    .child(div().text_xs().text_color(muted).child(
                        "Glob patterns separated by commas. Leave empty to include everything.",
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_3()
                            .child(size_field("Min size", &self.min_size, min_unit_group))
                            .child(size_field("Max size", &self.max_size, max_unit_group)),
                    ),
            );

        if let Some(error) = &self.error {
            content = content.child(div().text_sm().text_color(danger).child(error.clone()));
        }

        content = content.child(
            div().pt_4().border_t_1().border_color(border).child(
                Button::new("choose-directory")
                    .primary()
                    .w_full()
                    .icon(Icon::new(IconName::FolderOpen))
                    .label(if self.picking {
                        "Choosing…"
                    } else {
                        "Choose a directory to start…"
                    })
                    .disabled(self.picking)
                    .on_click(cx.listener(|this, _, _, cx| this.choose_directory(cx))),
            ),
        );

        div()
            .id("home-scroll")
            .size_full()
            .overflow_y_scroll()
            .bg(palette_card())
            .child(
                div()
                    .w_full()
                    .min_h_full()
                    .flex()
                    .flex_col()
                    .p(px(28.))
                    .child(content),
            )
            .into_any_element()
    }

    fn render_mode_toggle(&self, cx: &mut Context<Self>) -> AnyElement {
        let border = palette_border();
        let muted = palette_muted();

        div()
            .flex()
            .items_center()
            .justify_between()
            .gap_3()
            .p(px(10.))
            .px(px(12.))
            .border_1()
            .border_color(border)
            .rounded(px(10.))
            .bg(palette_subtle())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .flex_1()
                    .min_w_0()
                    .child(div().text_sm().font_semibold().child("Quick mode"))
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .whitespace_normal()
                            .child(
                                "Skip full hashing. Files up to 4 KB are fully hashed; larger ones use a 4 KB sample and may be wrong.",
                            ),
                    ),
            )
            .child(div().flex_none().child(
                Switch::new("quick-mode")
                    .checked(self.quick_mode)
                    .on_change(cx.listener(|this, checked, _, cx| {
                        this.quick_mode = *checked;
                        cx.notify();
                    })),
            ))
            .into_any_element()
    }

    fn render_analysis(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(run) = self.run.as_ref() else {
            return div().into_any_element();
        };

        let cancelling = run.cancelling;
        let border = palette_border();

        let steps = if self.quick_mode { 2 } else { 3 };
        let stage = run.stage.min(steps);
        // Each step owns the bar: it runs 0..100% for the current stage and
        // resets when the next stage begins.
        let step_progress = match stage {
            0 => None,
            1 => Some(progress_fraction(run.quick)),
            2 if self.quick_mode => Some(1.0),
            2 => Some(progress_fraction(run.full)),
            _ => Some(1.0),
        };
        let indeterminate = step_progress.is_none();
        let percent = step_progress.unwrap_or(0.0) * 100.0;

        let active_step = stage.min(steps - 1);
        let stepper = render_stepper(steps, active_step);

        let mut bar = Progress::new("analysis-progress")
            .value(percent)
            .w_full()
            .color(step_color(active_step));
        if indeterminate {
            bar = bar.loading(true);
        }

        let card = div()
            .w_full()
            .max_w(px(720.))
            .mx_auto()
            .flex()
            .flex_col()
            .gap_3()
            .p(px(26.))
            .bg(palette_card())
            .border_1()
            .border_color(border)
            .rounded(px(14.))
            .shadow_md()
            .child(stepper)
            .child(bar);

        let status = if cancelling { "CANCELLING" } else { "SCANNING" };
        let content = div()
            .size_full()
            .max_w(px(960.))
            .mx_auto()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        Button::new("cancel-analysis")
                            .h(px(42.))
                            .px(px(12.))
                            .rounded(px(9.))
                            .bg(palette_card())
                            .border_1()
                            .border_color(palette_border())
                            .shadow_sm()
                            .label(if cancelling {
                                "Cancelling…"
                            } else {
                                "Cancel"
                            })
                            .icon(Icon::new(IconName::X))
                            .disabled(cancelling)
                            .on_click(cx.listener(|this, _, _, cx| this.cancel_analysis(cx))),
                    )
                    .child(path_pill(&run.root, status)),
            )
            .child(card);

        div()
            .size_full()
            .flex()
            .flex_col()
            .p(px(24.))
            .child(content)
            .into_any_element()
    }

    fn render_results(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(results) = self.results.as_ref() else {
            return div().into_any_element();
        };

        let danger = palette_danger();

        let duplicate_groups = results
            .groups
            .iter()
            .filter(|group| group.files.len() > 1)
            .count();
        let redundant_files: usize = results
            .groups
            .iter()
            .map(|group| group.files.len().saturating_sub(1))
            .sum();
        let reclaimable: u64 = results
            .groups
            .iter()
            .map(|group| group.size_bytes * group.files.len().saturating_sub(1) as u64)
            .sum();
        let reclaimable_text = core::to_human_readable_size(reclaimable);

        let mut content = div()
            .size_full()
            .max_w(px(960.))
            .mx_auto()
            .min_h_0()
            .flex()
            .flex_col()
            .gap_3();

        content = content.child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .child(
                    Button::new("back")
                        .h(px(42.))
                        .px(px(12.))
                        .rounded(px(9.))
                        .bg(palette_card())
                        .border_1()
                        .border_color(palette_border())
                        .shadow_sm()
                        .label("Back")
                        .icon(Icon::new(IconName::ArrowLeft))
                        .on_click(cx.listener(|this, _, _, cx| this.go_back(cx))),
                )
                .child(path_pill(&results.root, "SCANNED")),
        );

        if results.quick_mode {
            content = content.child(banner(
                IconName::TriangleAlert,
                palette_warning_soft(),
                rgba(0xb453093d),
                palette_warning(),
                "Quick mode: files larger than 4 KB are matched by a 4 KB sample hash. Verify sampled groups with a full hash before deleting files.",
            ));
        }

        if !results.skipped.is_empty() {
            content = content.child(banner(
                IconName::TriangleAlert,
                palette_warning_soft(),
                rgba(0xb453093d),
                palette_warning(),
                &format!(
                    "{} files could not be read and were skipped.",
                    results.skipped.len()
                ),
            ));
        }

        if let Some(error) = &self.error {
            content = content.child(banner(
                IconName::CircleAlert,
                palette_danger_soft(),
                rgba(0xdc26263d),
                danger,
                error,
            ));
        }

        let mut visible_indices = Vec::new();
        let mut item_sizes = Vec::new();
        // The stats row is the first virtual item so it scrolls with the list.
        item_sizes.push(size(px(0.), px(STATS_ITEM_HEIGHT)));
        for (index, group) in results.groups.iter().enumerate() {
            let visible =
                group.files.len() > 1 || matches!(&group.verification, Verification::NoDuplicates);
            if !visible {
                continue;
            }
            visible_indices.push(index);
            item_sizes.push(size(px(0.), px(group_slot_height(group))));
        }
        let visible_indices = Rc::new(visible_indices);

        // Only build the cards that intersect the viewport. Rendering every
        // group up front made the first paint and each scroll freeze on large
        // result sets.
        let list = div()
            .flex_1()
            .min_h_0()
            .relative()
            .child(
                v_virtual_list(
                    cx.entity(),
                    "groups-list",
                    Rc::new(item_sizes),
                    move |this, range, _window, cx| {
                        let Some(results) = this.results.as_ref() else {
                            return Vec::new();
                        };
                        range
                            .filter_map(|slot| {
                                if slot == 0 {
                                    return Some(stats_row(
                                        duplicate_groups,
                                        redundant_files,
                                        reclaimable_text.clone(),
                                    ));
                                }
                                let index = *visible_indices.get(slot - 1)?;
                                let group = results.groups.get(index)?;
                                let pending = results.pending_delete.as_ref().and_then(
                                    |(pending_index, file)| {
                                        (*pending_index == index).then_some(file.as_str())
                                    },
                                );
                                Some(render_group(index, group, results.quick_mode, pending, cx))
                            })
                            .collect::<Vec<_>>()
                    },
                )
                .track_scroll(&results.scroll),
            )
            .child(Scrollbar::vertical(&results.scroll));

        content = content.child(list);

        div()
            .size_full()
            .flex()
            .flex_col()
            .p(px(24.))
            .child(content)
            .into_any_element()
    }
}

impl Render for DedupperApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let background = linear_gradient(
            160.0,
            linear_color_stop(rgb(0xf4f6fb), 0.0),
            linear_color_stop(rgb(0xeef2fd), 1.0),
        );
        let mut root = div()
            .size_full()
            .flex()
            .flex_col()
            .bg(background)
            .text_color(palette_text());

        root = match self.screen {
            Screen::Home => root.child(self.render_home(cx)),
            Screen::Analysis => root.child(self.render_analysis(cx)),
            Screen::Results => root.child(self.render_results(cx)),
        };

        root
    }
}

fn input_field(label: &'static str, input: &Entity<InputState>) -> AnyElement {
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_xs()
                .font_medium()
                .text_color(rgb(0x334155))
                .child(label),
        )
        .child(Input::new(input).w_full())
        .into_any_element()
}

fn size_field(label: &'static str, input: &Entity<InputState>, units: TabBar) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w(px(240.))
        .gap_1()
        .child(
            div()
                .text_xs()
                .font_medium()
                .text_color(rgb(0x334155))
                .child(label),
        )
        .child(Input::new(input).w_full().suffix(units))
        .into_any_element()
}

fn path_pill(root: &std::path::Path, status: &'static str) -> AnyElement {
    let muted = palette_muted();
    div()
        .flex()
        .items_center()
        .gap_2()
        .flex_1()
        .min_w_0()
        .h(px(42.))
        .px(px(12.))
        .bg(palette_card())
        .border_1()
        .border_color(palette_border())
        .rounded(px(9.))
        .shadow_sm()
        .child(
            Icon::new(IconName::FolderOpen)
                .size(px(15.))
                .text_color(muted),
        )
        .child(
            div()
                .text_xs()
                .font_semibold()
                .text_color(muted)
                .child(status),
        )
        .child(
            div()
                .text_sm()
                .min_w_0()
                .text_ellipsis()
                .child(root.to_string_lossy().to_string()),
        )
        .into_any_element()
}

fn stats_row(duplicate_groups: usize, redundant_files: usize, reclaimable: String) -> AnyElement {
    div()
        .w_full()
        .pr(px(SCROLLBAR_GUTTER))
        .flex()
        .gap_3()
        .child(stat("duplicate groups", duplicate_groups.to_string()))
        .child(stat("redundant files", redundant_files.to_string()))
        .child(stat("can be reclaimed", reclaimable))
        .into_any_element()
}

fn stat(label: &'static str, value: String) -> AnyElement {
    div()
        .flex_1()
        .flex()
        .flex_col()
        .gap_1()
        .p(px(13.))
        .px(px(16.))
        .bg(palette_card())
        .border_1()
        .border_color(palette_border())
        .rounded(px(11.))
        .shadow_sm()
        .child(
            div()
                .text_xl()
                .line_height(px(28.))
                .font_semibold()
                .child(value),
        )
        .child(
            div()
                .text_xs()
                .line_height(px(17.))
                .whitespace_nowrap()
                .text_ellipsis()
                .text_color(palette_muted())
                .child(label),
        )
        .into_any_element()
}

fn progress_fraction(progress: Option<(usize, usize)>) -> f32 {
    match progress {
        Some((done, total)) if total > 0 => (done as f32 / total as f32).clamp(0.0, 1.0),
        Some(_) => 1.0,
        None => 0.0,
    }
}

fn step_color(step: usize) -> gpui_kit::Rgba {
    match step {
        0 => rgb(0x16a34a),
        1 => rgb(0x0ea5e9),
        _ => rgb(0x8b5cf6),
    }
}

fn step_icon(step: usize) -> IconName {
    match step {
        0 => IconName::FolderSearch,
        1 => IconName::ScanSearch,
        _ => IconName::Hash,
    }
}

fn step_label(step: usize) -> &'static str {
    match step {
        0 => "Scanning",
        1 => "Quick filter",
        _ => "Full hash",
    }
}

/// Custom stepper so every step can own a distinct color. The progress bar
/// below uses the color of the active step.
fn render_stepper(steps: usize, active: usize) -> AnyElement {
    let mut row = div().w_full().flex().items_start();
    for step in 0..steps {
        let color = step_color(step);
        let reached = step <= active;

        row = row.child(
            div()
                .flex()
                .flex_shrink_0()
                .flex_col()
                .items_center()
                .gap_1()
                .child(
                    div()
                        .size(px(32.))
                        .rounded(px(999.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(if reached { color } else { color.opacity(0.16) })
                        .child(
                            Icon::new(step_icon(step))
                                .size(px(16.))
                                .text_color(if reached { rgb(0xffffff) } else { color }),
                        ),
                )
                .child(
                    div()
                        .text_xs()
                        .whitespace_nowrap()
                        .text_color(if reached {
                            palette_text()
                        } else {
                            palette_muted()
                        })
                        .child(step_label(step)),
                ),
        );

        if step + 1 < steps {
            row = row.child(div().flex_1().h(px(2.)).mt(px(15.)).bg(if step < active {
                step_color(step)
            } else {
                palette_border()
            }));
        }
    }
    row.into_any_element()
}

const GROUP_SPACING: f32 = 12.0;
const STATS_ITEM_HEIGHT: f32 = 90.0;
const SCROLLBAR_GUTTER: f32 = 14.0;
const GROUP_BORDER: f32 = 2.0;
const GROUP_PADDING_V: f32 = 28.0;
const GROUP_HEADER_HEIGHT: f32 = 33.0;
const GROUP_STATUS_HEIGHT: f32 = 31.0;
const GROUP_FILE_HEIGHT: f32 = 53.0;
const GROUP_FILE_NO_DIR_HEIGHT: f32 = 40.0;
const GROUP_CHILD_GAP: f32 = 8.0;

/// Height reserved for one group card in the virtual list.
///
/// The card layout uses fixed line heights and single-line rows, so this is an
/// upper bound; the extra `GROUP_SPACING` becomes the gap between cards.
fn group_slot_height(group: &Group) -> f32 {
    let has_status = matches!(
        &group.verification,
        Verification::Done { .. } | Verification::NoDuplicates | Verification::Error(_)
    );
    let files = group.files.len();
    let children = 1 + files + if has_status { 1 } else { 0 };
    let gaps = GROUP_CHILD_GAP * children.saturating_sub(1) as f32;
    let files_height: f32 = group
        .files
        .iter()
        .map(|path| {
            if path.rfind(|c| c == '\\' || c == '/').is_some() {
                GROUP_FILE_HEIGHT
            } else {
                GROUP_FILE_NO_DIR_HEIGHT
            }
        })
        .sum();

    GROUP_SPACING
        + GROUP_BORDER
        + GROUP_PADDING_V
        + GROUP_HEADER_HEIGHT
        + if has_status { GROUP_STATUS_HEIGHT } else { 0.0 }
        + files_height
        + gaps
}

fn render_group(
    index: usize,
    group: &Group,
    quick_mode: bool,
    pending: Option<&str>,
    cx: &mut Context<DedupperApp>,
) -> AnyElement {
    let muted = palette_muted();
    let border = palette_border();

    let mut card = div()
        .w_full()
        .flex()
        .flex_col()
        .gap_2()
        .p(px(14.))
        .bg(palette_card())
        .border_1()
        .border_color(border)
        .rounded(px(14.))
        .shadow_sm();

    let badge = if group.files.is_empty() {
        div()
            .flex()
            .flex_shrink_0()
            .items_center()
            .gap_1()
            .px(px(9.))
            .py(px(2.))
            .rounded(px(999.))
            .bg(palette_danger_soft())
            .text_color(palette_danger())
            .child(
                Icon::new(IconName::TriangleAlert)
                    .size(px(12.))
                    .text_color(palette_danger()),
            )
            .child(
                div()
                    .text_xs()
                    .font_semibold()
                    .line_height(px(17.))
                    .whitespace_nowrap()
                    .child("Not duplicates"),
            )
            .into_any_element()
    } else {
        div()
            .flex()
            .flex_shrink_0()
            .items_center()
            .gap_1()
            .px(px(9.))
            .py(px(2.))
            .rounded(px(999.))
            .bg(palette_accent_soft())
            .text_color(palette_accent_deep())
            .child(
                Icon::new(IconName::Copy)
                    .size(px(12.))
                    .text_color(palette_accent_deep()),
            )
            .child(
                div()
                    .text_xs()
                    .font_semibold()
                    .line_height(px(17.))
                    .whitespace_nowrap()
                    .child(format!("{} copies", group.files.len())),
            )
            .into_any_element()
    };

    let mut left = div()
        .flex()
        .flex_1()
        .items_center()
        .gap_2()
        .min_w_0()
        .child(badge);
    if !group.files.is_empty() {
        let (hash, sampled) = match &group.verification {
            Verification::Done { hash, .. } => (hash.clone(), false),
            _ => (group.hash.clone(), quick_mode && group.sampled),
        };
        left = left.child(
            div()
                .text_xs()
                .min_w_0()
                .text_ellipsis()
                .whitespace_nowrap()
                .line_height(px(17.))
                .text_color(muted)
                .child(hash),
        );
        if sampled {
            left = left.child(
                div()
                    .flex_shrink_0()
                    .px(px(5.))
                    .py(px(1.))
                    .rounded(px(999.))
                    .bg(palette_warning_soft())
                    .text_color(palette_warning())
                    .text_xs()
                    .line_height(px(17.))
                    .whitespace_nowrap()
                    .child("sample"),
            );
        }
    }

    let can_verify = quick_mode
        && group.sampled
        && !group.files.is_empty()
        && !matches!(
            &group.verification,
            Verification::Done { .. } | Verification::NoDuplicates
        );
    let checking = matches!(&group.verification, Verification::Checking);

    let mut right = div().flex().flex_shrink_0().items_center().gap_2().child(
        div()
            .text_xs()
            .line_height(px(17.))
            .whitespace_nowrap()
            .text_color(muted)
            .child(group.size.clone()),
    );
    if can_verify {
        right = right.child(
            Button::new(format!("verify-{index}"))
                .small()
                .label(if checking { "Hashing…" } else { "Full hash" })
                .disabled(checking)
                .on_click(cx.listener(move |this, _, _, cx| this.verify_group(index, cx))),
        );
    }

    card = card.child(
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap_2()
            .pb_2()
            .border_b_1()
            .border_color(border)
            .child(left)
            .child(right),
    );

    let status = match &group.verification {
        Verification::Idle | Verification::Checking => None,
        Verification::Done { hash, removed } => Some((
            true,
            if *removed > 0 {
                format!(
                    "Full hash verified: {hash} ({removed} non-duplicate {} removed)",
                    if *removed == 1 { "file" } else { "files" }
                )
            } else {
                format!("Full hash verified: {hash}")
            },
        )),
        Verification::NoDuplicates => Some((
            false,
            "Full hashes differ — these files are not duplicates.".to_string(),
        )),
        Verification::Error(message) => {
            Some((false, format!("Could not compute full hashes: {message}")))
        }
    };
    if let Some((ok, text)) = status {
        card = card.child(
            div()
                .flex()
                .items_center()
                .gap_1()
                .px(px(10.))
                .py(px(7.))
                .rounded(px(8.))
                .bg(if ok {
                    palette_success_soft()
                } else {
                    palette_danger_soft()
                })
                .text_color(if ok {
                    palette_success()
                } else {
                    palette_danger()
                })
                .child(
                    Icon::new(if ok {
                        IconName::CircleCheck
                    } else {
                        IconName::TriangleAlert
                    })
                    .size(px(12.))
                    .text_color(if ok {
                        palette_success()
                    } else {
                        palette_danger()
                    }),
                )
                .child(
                    div()
                        .text_xs()
                        .line_height(px(17.))
                        .flex_1()
                        .min_w_0()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .child(text),
                ),
        );
    }

    for (file_index, file) in group.files.iter().enumerate() {
        let is_pending = pending == Some(file.as_str());
        let (name, dir) = path_parts(file);
        let mut actions = div().flex().flex_shrink_0().gap_2();

        if is_pending {
            actions = actions
                .child(
                    Button::new(format!("confirm-delete-{index}-{file_index}"))
                        .small()
                        .h(px(24.))
                        .danger()
                        .outline()
                        .label("Confirm delete")
                        .on_click(cx.listener(|this, _, _, cx| this.confirm_delete(cx))),
                )
                .child(
                    Button::new(format!("cancel-delete-{index}-{file_index}"))
                        .small()
                        .h(px(24.))
                        .secondary()
                        .outline()
                        .label("Cancel")
                        .on_click(cx.listener(|this, _, _, cx| this.cancel_delete(cx))),
                );
        } else {
            let reveal_path = file.clone();
            let delete_path = file.clone();
            actions = actions
                .child(
                    Button::new(format!("reveal-{index}-{file_index}"))
                        .small()
                        .h(px(24.))
                        .secondary()
                        .outline()
                        .label("Reveal")
                        .icon(Icon::new(IconName::FolderOpen))
                        .on_click(
                            cx.listener(move |this, _, _, cx| this.reveal(reveal_path.clone(), cx)),
                        ),
                )
                .child(
                    Button::new(format!("delete-{index}-{file_index}"))
                        .small()
                        .h(px(24.))
                        .danger()
                        .outline()
                        .label("Delete")
                        .icon(Icon::new(IconName::Trash))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.request_delete(index, delete_path.clone(), cx)
                        })),
                );
        }

        let mut text = div().flex().flex_col().min_w_0().child(
            div()
                .text_sm()
                .line_height(px(20.))
                .min_w_0()
                .text_ellipsis()
                .whitespace_nowrap()
                .child(name),
        );
        if !dir.is_empty() {
            text = text.child(
                div()
                    .text_xs()
                    .line_height(px(17.))
                    .min_w_0()
                    .text_ellipsis()
                    .whitespace_nowrap()
                    .text_color(muted)
                    .child(dir),
            );
        }

        card = card.child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .px(px(10.))
                .py(px(8.))
                .child(
                    div()
                        .flex()
                        .flex_1()
                        .items_center()
                        .gap_2()
                        .min_w_0()
                        .child(Icon::new(IconName::File).size(px(18.)).text_color(muted))
                        .child(text),
                )
                .child(actions),
        );
    }

    div()
        .w_full()
        .pr(px(SCROLLBAR_GUTTER))
        .child(card)
        .into_any_element()
}

fn banner(
    icon: IconName,
    background: gpui_kit::Rgba,
    border: gpui_kit::Rgba,
    color: gpui_kit::Rgba,
    text: &str,
) -> AnyElement {
    div()
        .flex()
        .items_start()
        .gap_2()
        .p(px(11.))
        .px(px(14.))
        .bg(background)
        .border_1()
        .border_color(border)
        .rounded(px(10.))
        .text_color(color)
        .child(
            div()
                .flex()
                .flex_shrink_0()
                .items_center()
                .h(px(20.))
                .child(Icon::new(icon).size(px(15.)).text_color(color)),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .whitespace_normal()
                .text_sm()
                .line_height(px(20.))
                .child(text.to_string()),
        )
        .into_any_element()
}

fn path_parts(path: &str) -> (String, String) {
    match path.rfind(|c| c == '\\' || c == '/') {
        Some(index) => (path[index + 1..].to_string(), path[..index].to_string()),
        None => (path.to_string(), String::new()),
    }
}

fn palette_card() -> gpui_kit::Rgba {
    rgb(0xffffff)
}

fn palette_subtle() -> gpui_kit::Rgba {
    rgb(0xf6f8fe)
}

fn palette_text() -> gpui_kit::Rgba {
    rgb(0x0f172a)
}

fn palette_muted() -> gpui_kit::Rgba {
    rgb(0x64748b)
}

fn palette_border() -> gpui_kit::Rgba {
    rgba(0x0f172a14)
}

fn palette_accent_deep() -> gpui_kit::Rgba {
    rgb(0x3567e0)
}

fn palette_accent_soft() -> gpui_kit::Rgba {
    rgba(0x4f83ff17)
}

fn palette_danger() -> gpui_kit::Rgba {
    rgb(0xdc2626)
}

fn palette_danger_soft() -> gpui_kit::Rgba {
    rgba(0xdc262614)
}

fn palette_success() -> gpui_kit::Rgba {
    rgb(0x16a34a)
}

fn palette_success_soft() -> gpui_kit::Rgba {
    rgba(0x16a34a1a)
}

fn palette_warning() -> gpui_kit::Rgba {
    rgb(0xb45309)
}

fn palette_warning_soft() -> gpui_kit::Rgba {
    rgba(0xb4530914)
}

fn parse_size_value(input: &str) -> Option<f64> {
    let value: f64 = input.trim().parse().ok()?;
    if !value.is_finite() || value < 0.0 {
        return None;
    }
    Some(value)
}

pub fn run() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::AllAssets)
        .run(|cx| {
            gpui_kit::init(cx);
            Theme::change(ThemeMode::Light, None, cx);
            Theme::update(cx, |theme| {
                theme.radius = px(10.);
                theme.background = rgb(0xf4f6fb).into();
                theme.foreground = rgb(0x0f172a).into();
                theme.border = rgba(0x0f172a1f).into();
                theme.input = rgb(0xffffff).into();
                theme.muted = rgb(0xf6f8fe).into();
                theme.muted_foreground = rgb(0x64748b).into();
                theme.primary = rgb(0x4f83ff).into();
                theme.primary_hover = rgb(0x3567e0).into();
                theme.primary_active = rgb(0x2f5bcc).into();
                theme.primary_foreground = rgb(0xffffff).into();
                theme.secondary = rgb(0xf6f8fe).into();
                theme.secondary_hover = rgb(0xeef2fd).into();
                theme.secondary_active = rgb(0xe2ebff).into();
                theme.secondary_foreground = rgb(0x0f172a).into();
                theme.button = rgb(0xf6f8fe).into();
                theme.button_hover = rgb(0xeef2fd).into();
                theme.button_active = rgb(0xe2ebff).into();
                theme.button_foreground = rgb(0x0f172a).into();
                theme.button_primary = rgb(0x4f83ff).into();
                theme.button_primary_hover = rgb(0x3567e0).into();
                theme.button_primary_active = rgb(0x2f5bcc).into();
                theme.button_primary_foreground = rgb(0xffffff).into();
                theme.button_secondary = rgb(0xf6f8fe).into();
                theme.button_secondary_hover = rgb(0xeef2fd).into();
                theme.button_secondary_active = rgb(0xe2ebff).into();
                theme.button_secondary_foreground = rgb(0x0f172a).into();
                theme.danger = rgb(0xdc2626).into();
                theme.danger_hover = rgb(0xb91c1c).into();
                theme.danger_active = rgb(0x991b1b).into();
                theme.danger_foreground = rgb(0xffffff).into();
                theme.button_danger = rgb(0xdc2626).into();
                theme.button_danger_hover = rgb(0xb91c1c).into();
                theme.button_danger_active = rgb(0x991b1b).into();
                theme.button_danger_foreground = rgb(0xffffff).into();
                theme.success = rgb(0x16a34a).into();
                theme.warning = rgb(0xb45309).into();
                theme.progress_bar = rgb(0x4f83ff).into();
                theme.ring = rgb(0x4f83ff).into();
                theme.shadow = true;
            });
            let min_size = size(px(560.), px(680.));
            let bounds = WindowBounds::centered(min_size, cx);
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(bounds),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Deduplix".into()),
                        ..Default::default()
                    }),
                    window_min_size: Some(min_size),
                    ..Default::default()
                },
                cx,
                |window, cx| {
                    let app = cx.new(|cx| DedupperApp::new(window, cx));
                    app.update(cx, |app, cx| app.start_event_pump(cx));
                    app
                },
            )
            .expect("failed to open window");
        });
}
