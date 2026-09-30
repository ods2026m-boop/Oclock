//! The shell: the window's frame.
//!
//! Sidebar or top bar, a page header, the scrolling content, and the layers
//! that sit above all of it — dialogs, toasts, the shortcut list, the theme
//! veil. Keeping the frame in one place is what lets every page be written as
//! "the content", with none of them having to know where it is going.

use iced::widget::{column, container, row, Space};
use iced::{Background, Color, Element, Length, Shadow};

use crate::app::message::Message;
use crate::app::view::Layout;
use crate::app::OClock;
use crate::design::motion::ease;
use crate::design::palette::{Palette, ShadowLevel};
use crate::design::tokens::{layout, radius, space, stroke};
use crate::services::i18n::Text;
use crate::ui::components::dialog::{self, EmptyKind};
use crate::ui::components::icons::Icon;
use crate::ui::components::surfaces;
use crate::ui::typography::{self, Role};

/// The focus index of the appearance control.
///
/// The five destinations come first in reading order, and the appearance control
/// is drawn in every layout — at the foot of the rail or at the end of the top
/// bar — so it keeps this number whichever of them is on screen.
pub const APPEARANCE_FOCUS: usize = 5;

/// The focus index of the language control.
///
/// Beside the appearance control, and beside nothing at all in a top bar, which
/// has room for the destinations and the appearance control and no more.
pub const LANGUAGE_FOCUS: usize = 6;

/// The rendered window.
pub fn view(app: &OClock) -> Element<'static, Message> {
    let palette = app.palette();
    let frame = shell(app, palette);

    // The theme veil covers everything, including the dialogs, so the palette
    // swap is never visible halfway through an interaction.
    match app.theme.veil(&app.clock) {
        Some(colour) => surfaces::scrim(colour, frame),
        None => frame,
    }
}

/// The window's frame: navigation, header, content and overlays.
fn shell(app: &OClock, palette: Palette) -> Element<'static, Message> {
    let style = move |_theme: &iced::Theme| iced::widget::container::Style {
        background: Some(Background::Color(palette.canvas)),
        ..iced::widget::container::Style::default()
    };

    // The frame and the overlays are *stacked*, not laid out side by side. An
    // overlay as a row sibling takes its own width, which squeezes the page
    // underneath it — the header wraps, the cards narrow, and a dialog ends up
    // off the edge of the window.
    let body = content(app, palette);
    let frame: Element<'static, Message> = match rail(app, palette) {
        // A rail runs down the leading edge, so it comes first in reading order:
        // the left in a left-to-right language, the right in a right-to-left
        // one.
        Some(rail) => container(crate::ui::ends(app.catalog.direction(), rail, body).spacing(0.0))
            .width(Length::Fill)
            .height(Length::Fill)
            .into(),

        // With no room for a rail the destinations go into a bar across the
        // top — and a bar across the top goes *above* the content, not beside
        // it. In the same row it would take half the window's width and push
        // the page into what was left, which on a narrow window is not a
        // layout but a squeeze.
        None => container(
            column![top_bar(app, palette), body]
                .spacing(0.0)
                .width(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .into(),
    };

    let mut stack = iced::widget::Stack::new();
    stack = stack.push(frame);
    stack = stack.push(overlays(app, palette));

    container(stack)
        .style(style)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// The five destinations, in the form the current layout wants them.
fn destinations(app: &OClock) -> Vec<Element<'static, Message>> {
    app.navigation.view(
        app.palette(),
        app.catalog,
        &app.clock,
        &app.interactions,
        app.view.layout.is_expanded(),
        app.view.layout.has_sidebar(),
        app.view.focus,
        Message::Navigate,
    )
}

/// The navigation rail, or `None` when the window is too narrow for one.
fn rail(app: &OClock, palette: Palette) -> Option<Element<'static, Message>> {
    if !app.view.layout.has_sidebar() {
        return None;
    }

    let items = destinations(app);

    let mut contents = column![]
        .spacing(space::XS)
        .width(Length::Fill)
        .align_x(crate::ui::leading(app.catalog.direction()))
        .push(brand(app, palette))
        .push(Space::with_height(space::MD))
        .push(
            column(items)
                .spacing(space::XXS)
                .width(Length::Fill)
                .align_x(crate::ui::leading(app.catalog.direction())),
        )
        .push(Space::with_height(space::LG))
        .push(appearance_control(app, palette))
        .push(Space::with_height(space::XS))
        .push(language_control(app, palette));

    if app.view.layout.is_expanded() {
        // The shortcut hint only appears where there is room for it. The `?` is
        // a key name and stays beside the words, on the side it was written for
        // the language rather than for the keyboard.
        let hint = if app.is_rtl() {
            format!("?  {}", app.tr(Text::ShortcutsTitle))
        } else {
            format!("{}  ?", app.tr(Text::ShortcutsTitle))
        };
        contents = contents.push(Space::with_height(space::SM)).push(
            typography::Label::new(hint, Role::Micro, palette.text_faint)
                .proportional()
                .reading(app.catalog)
                .natural()
                .view(),
        );
    }

    let style = move |_theme: &iced::Theme| iced::widget::container::Style {
        background: Some(Background::Color(palette.canvas)),
        border: crate::ui::components::navigation::rail_border(palette),
        shadow: palette.shadow(ShadowLevel::Medium),
        ..iced::widget::container::Style::default()
    };

    let sidebar: Element<'static, Message> = container(contents.padding(iced::padding::all(
        if app.view.layout.is_expanded() {
            space::LG
        } else {
            space::SM
        },
    )))
    .style(style)
    .width(Length::Fixed(if app.view.layout.is_expanded() {
        layout::SIDEBAR
    } else {
        layout::SIDEBAR_COMPACT
    }))
    .height(Length::Fill)
    .into();

    Some(sidebar)
}

/// The brand: the application's mark and name.
fn brand(app: &OClock, palette: Palette) -> Element<'static, Message> {
    let name: Element<'static, Message> = if app.view.layout.is_expanded() {
        typography::Label::new(crate::APP_NAME, Role::Title, palette.text)
            .reading(app.catalog)
            .view()
            .into()
    } else {
        Space::with_width(1.0).into()
    };

    let mark: Element<'static, Message> =
        crate::ui::components::controls::mark(Icon::Clock, 22.0, palette.accent_text)
            .map(|()| unreachable!("the brand never sends a message"));

    let gutter: Element<'static, Message> = if app.view.layout.is_expanded() {
        Space::with_width(space::MD).into()
    } else {
        Space::with_width(0.0).into()
    };

    // The mark leads the name, so it takes the leading edge of the row.
    crate::ui::sequence(app.catalog.direction(), mark, gutter, name)
        .align_y(iced::Alignment::Center)
        .into()
}

/// What pressing the appearance control sends.
///
/// The control names the appearance it will move to rather than merely cycling
/// blindly, so the keyboard has to ask the same question the control answered:
/// one definition of "what pressing it does", used by both.
pub fn appearance_message(app: &OClock) -> Message {
    Message::ChooseAppearance(app.mode.next())
}

/// What pressing the language control sends: the language it will move to.
pub fn language_message(app: &OClock) -> Message {
    Message::ChooseLanguage(app.language.next())
}

/// The appearance control: a compact switch between light, dark and the
/// desktop's own preference.
fn appearance_control(app: &OClock, palette: Palette) -> Element<'static, Message> {
    use crate::ui::components::controls::ChipSpec;

    // The control shows the appearance in effect and offers the next one, so it
    // always says what pressing it will do.
    let (label, icon): (Text, Icon) = match app.mode {
        crate::design::theme::ThemeMode::System => (Text::ThemeSystem, Icon::System),
        crate::design::theme::ThemeMode::Light => (Text::ThemeLight, Icon::Sun),
        crate::design::theme::ThemeMode::Dark => (Text::ThemeDark, Icon::Moon),
    };
    let label = app.tr(label);
    let message = appearance_message(app);

    if app.view.layout.is_expanded() {
        ChipSpec::new(
            app.interactions.get("shell.appearance"),
            app.clock.clone(),
            palette,
            typography::Label::new(label, Role::Label, palette.text_muted).reading(app.catalog),
            false,
            message,
        )
        .icon(icon)
        .focused(app.shell_focused(APPEARANCE_FOCUS))
        .view()
    } else {
        crate::ui::components::controls::IconButtonSpec::new(
            app.interactions.get("shell.appearance"),
            app.clock.clone(),
            palette,
            crate::ui::components::controls::Emphasis::Ghost,
            icon,
            message,
        )
        .focused(app.shell_focused(APPEARANCE_FOCUS))
        .view()
    }
}

/// The language control: cycles through the twelve languages.
///
/// A control that *cycles* rather than a menu because the rail is a column of
/// chips and a twelve-row menu would be a poor fit for a 236-pixel sidebar. The
/// chip names the language that pressing it will move to, so the control always
/// says what it will do, and the endonym is what it says it with — a reader
/// looking for their own language does not recognise it written in the language
/// they are currently reading.
///
/// The languages are listed in a fixed order rather than the current one plus
/// the next, so the sequence is the same whichever language it is cycled from.
fn language_control(app: &OClock, palette: Palette) -> Element<'static, Message> {
    use crate::ui::components::controls::ChipSpec;

    let next = app.language.next();
    let label = typography::Label::new(next.endonym(), Role::Label, palette.text_muted)
        .reading(app.catalog);
    let message = language_message(app);

    if app.view.layout.is_expanded() {
        ChipSpec::new(
            app.interactions.get("shell.language"),
            app.clock.clone(),
            palette,
            label,
            false,
            message,
        )
        .icon(Icon::Globe)
        .focused(app.shell_focused(LANGUAGE_FOCUS))
        .view()
    } else {
        crate::ui::components::controls::IconButtonSpec::new(
            app.interactions.get("shell.language"),
            app.clock.clone(),
            palette,
            crate::ui::components::controls::Emphasis::Ghost,
            Icon::Globe,
            message,
        )
        .focused(app.shell_focused(LANGUAGE_FOCUS))
        .view()
    }
}

/// A top bar, for a window too narrow for a rail.
fn top_bar(app: &OClock, palette: Palette) -> Element<'static, Message> {
    let items = destinations(app);
    let style = move |_theme: &iced::Theme| iced::widget::container::Style {
        background: Some(Background::Color(palette.canvas)),
        border: Border {
            color: palette.outline,
            width: stroke::HAIRLINE,
            radius: 0.0.into(),
        },
        shadow: palette.shadow(ShadowLevel::Low),
        ..iced::widget::container::Style::default()
    };

    let items: Element<'static, Message> = row(items).spacing(space::XXS).into();

    // Brand, then the destinations, then a gap, then the appearance control:
    // the reading order of a bar, whichever end the bar starts from.
    let direction = app.catalog.direction();
    let lead: Element<'static, Message> = crate::ui::sequence(
        direction,
        brand(app, palette),
        Space::with_width(space::MD),
        items,
    )
    .into();
    let bar = crate::ui::sequence(
        direction,
        lead,
        Space::with_width(Length::Fill),
        appearance_control(app, palette),
    );

    container(
        bar.spacing(space::SM)
            .align_y(iced::Alignment::Center)
            .padding(iced::padding::all(space::MD)),
    )
    .style(style)
    .width(Length::Fill)
    .into()
}

use iced::Border;

/// The content column: a header and the page itself.
fn content(app: &OClock, palette: Palette) -> Element<'static, Message> {
    // Everything in the content column is explicitly Fill, so nothing can
    // shrink-wrap its way past the window's edge.
    let body = column![
        header(app, palette),
        surfaces::scroll_area(page(app, palette), palette),
    ]
    .spacing(0.0)
    .width(Length::Fill)
    .height(Length::Fill);

    container(body)
        .padding(iced::padding::left(space::XXL).right(space::XXL))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// The page header: the page's name, its one-line purpose, and its controls.
fn header(app: &OClock, palette: Palette) -> Element<'static, Message> {
    let destination = app
        .navigation
        .destination()
        .copied()
        .unwrap_or_else(|| Message::destinations()[0]);

    // The title takes the space it needs and wraps; the actions keep theirs.
    // A row gives every child its full intrinsic width unless something is
    // Fill, so without this a long summary pushes the actions off the edge.
    //
    // The name leads its one-line purpose, and the purpose is prose, so it is
    // given the remaining width and hangs from the trailing margin rather than
    // the leading one.
    let purpose = container(
        typography::Label::new(
            destination.summary_text(app.catalog),
            Role::Body,
            palette.text_faint,
        )
        .reading(app.catalog)
        .proportional()
        .view(),
    )
    .width(Length::Fill);

    let title: Element<'static, Message> = crate::ui::ends(
        app.catalog.direction(),
        container(
            typography::Label::new(
                destination.label_text(app.catalog),
                Role::Title,
                palette.text,
            )
            .reading(app.catalog)
            .proportional()
            .natural()
            .view(),
        )
        .width(Length::Shrink),
        purpose,
    )
    .spacing(space::MD)
    .align_y(iced::Alignment::End)
    .width(Length::Fill)
    .into();

    let bar = column![
        crate::ui::ends(app.catalog.direction(), title, page_actions(app, palette))
            .align_y(iced::Alignment::Center),
        Space::with_height(space::LG),
    ]
    .width(Length::Fill);

    let style = move |_theme: &iced::Theme| iced::widget::container::Style {
        background: Some(Background::Color(palette.canvas)),
        ..iced::widget::container::Style::default()
    };

    container(bar)
        .style(style)
        .padding(iced::padding::top(space::XXL))
        .into()
}

/// The controls that belong to the current page, shown in the header.
fn page_actions(app: &OClock, palette: Palette) -> Element<'static, Message> {
    use crate::ui::components::controls::{ButtonSpec, Emphasis, IconButtonSpec};

    match app.view.page {
        0 => row![IconButtonSpec::new(
            app.interactions.get("clock.analog"),
            app.clock.clone(),
            palette,
            Emphasis::Secondary,
            Icon::Clock,
            Message::ToggleAnalog,
        )
        .focused(app.view.focus == app.page_focus(2))
        .view(),]
        .spacing(space::SM)
        .into(),

        1 => row![
            ButtonSpec::new(
                app.interactions.get("world.add"),
                app.clock.clone(),
                palette,
                Emphasis::Primary,
                typography::label(app.tr(Text::ActionAddCity), &palette).reading(app.catalog),
                Message::ToggleCityPicker,
            )
            .icon(Icon::Plus)
            .focused(app.view.focus == app.page_focus(0))
            .view(),
            IconButtonSpec::new(
                app.interactions.get("world.sort"),
                app.clock.clone(),
                palette,
                Emphasis::Secondary,
                Icon::Sort,
                Message::CycleSort,
            )
            .focused(app.view.focus == app.page_focus(1))
            .view(),
        ]
        .spacing(space::SM)
        .into(),

        2 => ButtonSpec::new(
            app.interactions.get("alarms.add"),
            app.clock.clone(),
            palette,
            Emphasis::Primary,
            typography::label(app.tr(Text::ActionNewAlarm), &palette).reading(app.catalog),
            Message::NewAlarm,
        )
        .icon(Icon::Plus)
        .focused(app.view.focus == app.page_focus(0))
        .view(),

        3 => ButtonSpec::new(
            app.interactions.get("timer.new"),
            app.clock.clone(),
            palette,
            Emphasis::Primary,
            typography::label(app.tr(Text::ActionNewTimer), &palette).reading(app.catalog),
            Message::NewTimer,
        )
        .icon(Icon::Plus)
        .focused(app.view.focus == app.page_focus(0))
        .view(),

        _ => Space::with_width(1.0).into(),
    }
}

/// The current page, cross-fading with the one it replaced.
/// The room a right-to-left page leaves for the scroll bar Iced cannot move.
fn page(app: &OClock, palette: Palette) -> Element<'static, Message> {
    let progress = app.view.transition();
    let recede = |amount: f32| move |element| surfaces::recede(palette, element, amount);

    let entering = match app.view.page {
        0 => crate::ui::pages::clock::view(app, palette),
        1 => crate::ui::pages::world::view(app, palette),
        2 => crate::ui::pages::alarms::view(app, palette),
        3 => crate::ui::pages::timer::view(app, palette),
        _ => crate::ui::pages::stopwatch::view(app, palette),
    };

    // The track is drawn on the viewport's leading edge, so the page has to
    // leave room on whichever side that is in this language.
    let entering: Element<'static, Message> = container(entering)
        .padding(surfaces::scrollbar_inset(app.catalog.direction()))
        .width(Length::Fill)
        .into();

    // The page being left is still drawn, sliding away beneath the one
    // arriving. Both are in the same stack, so the movement reads as a change
    // of contents rather than a change of screen.
    let Some(leaving_page) = app.view.leaving else {
        return entering;
    };

    let leaving = match leaving_page {
        0 => crate::ui::pages::clock::view(app, palette),
        1 => crate::ui::pages::world::view(app, palette),
        2 => crate::ui::pages::alarms::view(app, palette),
        3 => crate::ui::pages::timer::view(app, palette),
        _ => crate::ui::pages::stopwatch::view(app, palette),
    };

    // The page being left slides away and recedes; the one arriving slides in
    // and rises out of the background.
    //
    // The direction is the reading direction. A page that arrives from the
    // right in a left-to-right language arrives from the left in a
    // right-to-left one, because the eye enters a page from the side it starts
    // reading on — and a transition that ignores this reads as the interface
    // going backwards in half the world's languages.
    let travel: f32 = if app.catalog.direction().is_rtl() {
        -1.0
    } else {
        1.0
    };

    let offset = (1.0 - app.view.leaving_progress) * PAGE_TRAVEL;
    let leaving = surfaces::slide(leaving, -travel * offset);
    let leaving = recede(1.0 - app.view.leaving_progress)(leaving);

    let offset = (1.0 - progress) * PAGE_TRAVEL;
    let entering = surfaces::slide(entering, travel * offset);
    let entering = recede(1.0 - progress)(entering);

    iced::widget::Stack::with_children(vec![leaving, entering]).into()
}

/// How far a page travels as it changes, in logical pixels.
///
/// Sixteen is about one and a half percent of the window's width: enough for
/// the change of contents to be seen as a movement rather than a cut, and small
/// enough that the reader is not waiting for something to arrive. Anything
/// larger stops being a transition and becomes the main event.
const PAGE_TRAVEL: f32 = 16.0;

/// Everything drawn above the page: dialogs, the shortcut list, toasts.
fn overlays(app: &OClock, palette: Palette) -> Element<'static, Message> {
    let mut layers: Vec<Element<'static, Message>> = Vec::new();

    if app.view.has_modal() {
        let presence = app.dialog.value(app.clock.now());
        layers.push(dialog::overlay(palette, modal(app, palette, presence)));
    }

    if app.view.shortcuts {
        layers.push(dialog::overlay(palette, shortcuts(app, palette)));
    }

    if !app.view.ringing.is_empty() {
        layers.push(dialog::overlay(
            palette,
            crate::ui::pages::alarms::ringing(app, palette),
        ));
    }

    if !app.view.toasts.is_empty() {
        layers.push(toasts(app, palette));
    }

    if layers.is_empty() {
        return Space::with_width(0.0).into();
    }

    // One full-window layer per overlay, deepest first.
    let mut stack = iced::widget::Stack::new();
    for layer in layers.into_iter().rev() {
        stack = stack.push(layer);
    }
    stack.width(Length::Fill).height(Length::Fill).into()
}

/// Which dialog is showing.
fn modal(app: &OClock, palette: Palette, presence: f32) -> Element<'static, Message> {
    if let Some(draft) = app.view.alarm_draft.as_ref() {
        return crate::ui::pages::alarms::editor(app, palette, draft, presence);
    }
    if let Some(draft) = app.view.timer_draft.as_ref() {
        return crate::ui::pages::timer::editor(app, palette, draft, presence);
    }
    if app.view.city_picker {
        return crate::ui::pages::world::picker(app, palette, presence);
    }
    if let Some(id) = app.view.pending_delete {
        return crate::ui::pages::alarms::confirm_delete(app, palette, id, presence);
    }
    Space::with_width(1.0).into()
}

/// The toasts, stacked at the bottom of the window.
fn toasts(app: &OClock, palette: Palette) -> Element<'static, Message> {
    let mut items: Vec<Element<'static, Message>> = Vec::new();
    for toast in &app.view.toasts {
        let presence = toast.presence.value(app.clock.now());
        items.push(
            dialog::Toast::<Message>::new(palette, presence, &toast.text)
                .tone(toast.tone.colour(palette))
                .view(),
        );
    }

    container(
        column![
            Space::with_height(Length::Fill),
            column(items).spacing(space::SM)
        ]
        .width(Length::Fill),
    )
    .padding(iced::padding::all(space::XXXL))
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

/// The keyboard shortcut list.
fn shortcuts(app: &OClock, palette: Palette) -> Element<'static, Message> {
    use crate::ui::components::controls::{ButtonSpec, Emphasis};

    // The keys are key names and stay as they are: a keyboard does not change
    // layout with the interface. The descriptions are prose and are translated.
    //
    // Every row here is a key the application really handles, and nothing else
    // is listed: this used to offer <kbd>Ctrl-D</kbd> for the appearance, which
    // the key handler does not do — it passes anything with a modifier to the
    // desktop — and a shortcut list that promises a key the application ignores
    // is worse than one that leaves it out. The appearance is on the keyboard in
    // any case: it is a control like any other, and <kbd>Tab</kbd> reaches it.
    let rows: Vec<(&str, Text)> = vec![
        ("1 – 5", Text::ShortcutGoTo),
        ("Tab / Shift-Tab", Text::ShortcutMoveFocus),
        ("Enter / Space", Text::ShortcutActivate),
        ("Escape", Text::ShortcutClose),
        ("S", Text::ShortcutStopwatch),
        ("L", Text::ShortcutLap),
        ("R", Text::ShortcutReset),
        ("N", Text::ShortcutNew),
        ("?", Text::ShortcutThisList),
    ];

    let mut items = Vec::with_capacity(rows.len());
    for (keys, description) in rows {
        // The key combo belongs on the leading side of its description, so in a
        // right-to-left interface the two swap places.
        let keys_label = typography::Label::new(keys, Role::Label, palette.accent_text)
            .reading(app.catalog)
            .view();
        let gap: Element<'static, Message> = Space::with_width(space::LG).into();
        let description_label =
            typography::Label::new(app.tr(description), Role::Body, palette.text_muted)
                .proportional()
                .reading(app.catalog)
                .view();
        // Built twice rather than shuffled, because a widget is a value that
        // cannot be laid out twice.
        let line = if app.is_rtl() {
            row![description_label, gap, keys_label]
        } else {
            row![keys_label, gap, description_label]
        };
        items.push(line.align_y(iced::Alignment::Center).into());
    }

    let close: Element<'static, Message> = ButtonSpec::new(
        app.interactions.get("shell.shortcuts.close"),
        app.clock.clone(),
        palette,
        Emphasis::Primary,
        typography::label(app.tr(Text::ActionClose), &palette).reading(app.catalog),
        Message::ToggleShortcuts,
    )
    .view();

    dialog::Dialog::<Message>::new(palette, 1.0)
        .reading(app.catalog)
        .width(crate::ui::pages::dialog_width(app, 420.0))
        .limit(crate::ui::pages::dialog_limit(app))
        .push(typography::heading(app.tr(Text::ShortcutsTitle), &palette).view())
        .push(column(items).spacing(space::SM))
        .footer(close)
        .view()
}

/// The empty state for a page with nothing in it.
pub fn nothing<M: 'static>(
    kind: EmptyKind,
    palette: Palette,
    heading: &str,
    detail: &str,
) -> Element<'static, M> {
    dialog::empty(kind, palette, heading, detail, None)
}

/// The shadow under the shell's own surfaces.
pub fn shell_shadow(palette: Palette) -> Shadow {
    palette.shadow(ShadowLevel::Medium)
}

/// A rule between two regions, tinted to the palette.
pub fn rule(palette: Palette) -> Element<'static, ()> {
    surfaces::divider::<()>(palette, false)
}

/// The colour a card is drawn in.
pub fn card_colour(palette: Palette) -> Color {
    palette.surface
}

/// The corner radius of a shell surface.
pub fn shell_radius() -> f32 {
    radius::LG
}

/// Vertical rhythm for a page's sections.
pub fn section_gap() -> f32 {
    space::XXL
}

/// Horizontal page padding, given the layout.
pub fn page_padding(layout: Layout) -> f32 {
    match layout {
        Layout::Stacked => space::LG,
        Layout::Compact => space::XL,
        Layout::Wide => layout::PAGE_PADDING,
    }
}

/// Eases a value for a fade.
pub fn fade(value: f32) -> Color {
    ease::fade(Color::WHITE, value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::view::Layout;

    #[test]
    fn the_shell_renders() {
        let app = crate::app::tests::sample();
        let _: Element<'static, Message> = view(&app);
    }

    #[test]
    fn the_shell_renders_in_every_layout() {
        let mut app = crate::app::tests::sample();
        for layout in [Layout::Wide, Layout::Compact, Layout::Stacked] {
            app.view.layout = layout;
            let _: Element<'static, Message> = view(&app);
        }
    }

    #[test]
    fn page_padding_grows_with_the_window() {
        assert!(
            page_padding(Layout::Stacked) < page_padding(Layout::Wide),
            "a narrow window should give its content more room relative to itself"
        );
        assert!(page_padding(Layout::Compact) < page_padding(Layout::Wide));
    }

    #[test]
    fn helpers_are_neutral() {
        let palette = Palette::DARK;
        let _ = shell_shadow(palette);
        let _ = rule(palette);
        assert_eq!(card_colour(palette), palette.surface);
        assert!(shell_radius() > 0.0);
        assert!(section_gap() > 0.0);
        assert!((fade(0.5).a - 0.5).abs() < 1.0e-3);
    }

    #[test]
    fn empty_states_have_a_helper() {
        use crate::services::i18n::Catalog;
        let _: Element<'static, Message> = nothing(
            EmptyKind::Nothing,
            Palette::LIGHT,
            Catalog::english().text(Text::EmptyLapsTitle),
            Catalog::english().text(Text::EmptyLapsDetail),
        );
    }
}
