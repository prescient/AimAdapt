//! The chart and the note on the results screen. What goes where is worked out in
//! `crate::results`; this draws it. Everything is redrawn from the run's extras on
//! every frame the results show, so the next run starts with a clean screen.

use aimadapt_core::results::CHART_RUNS;
use aimadapt_sim::Phase;
use bevy::ecs::hierarchy::ChildSpawnerCommands;
use bevy::prelude::*;
use bevy::text::FontSize;

use super::{Game, KeysText, OverlayText, fmt_value};
use crate::results::{self, PLOT_H, PLOT_W};

const DIM: Color = Color::srgba(1.0, 1.0, 1.0, 0.6);
const EARLIER_DOT: Color = Color::srgba(1.0, 1.0, 1.0, 0.55);
/// The health bar's green.
const THIS_DOT: Color = Color::srgb(0.35, 0.85, 0.45);
const GRID_LINE: Color = Color::srgba(1.0, 1.0, 1.0, 0.14);

/// The whole chart: labels, plot and caption.
#[derive(Component)]
pub(super) struct ChartBlock;

/// The whole note: title and body.
#[derive(Component)]
pub(super) struct NoteBlock;

/// One of the chart's dots, by its place from the left.
#[derive(Component)]
pub(super) struct ChartDot(usize);

/// The chart and the note, to show or hide.
type Blocks<'w, 's> = Query<'w, 's, (&'static mut Node, Has<ChartBlock>), Or<(With<ChartBlock>, With<NoteBlock>)>>;

/// The chart's dots. Kept apart from the blocks, which also have a `Node`.
type Dots<'w, 's> = Query<
    'w,
    's,
    (&'static ChartDot, &'static mut Node, &'static mut BackgroundColor),
    (Without<ChartBlock>, Without<NoteBlock>),
>;

/// The results and the key hints, to count their lines. Kept apart from the texts
/// this module writes.
type PanelTexts<'w, 's> = Query<'w, 's, &'static Text, (Or<(With<OverlayText>, With<KeysText>)>, Without<ExtrasText>)>;

/// The text nodes that change.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(super) enum ExtrasText {
    Top,
    Bottom,
    Caption,
    NoteTitle,
    NoteBody,
}

fn font(size: f32) -> TextFont {
    TextFont { font_size: FontSize::Px(size), ..default() }
}

/// Spawns the chart and the note under the results panel, both hidden.
pub(super) fn spawn(panel: &mut ChildSpawnerCommands) {
    let hidden = Node { display: Display::None, flex_direction: FlexDirection::Column, ..default() };
    panel
        .spawn((ChartBlock, Node { align_items: AlignItems::Center, margin: UiRect::top(px(16)), ..hidden.clone() }))
        .with_children(|chart| {
            chart
                .spawn(Node { align_items: AlignItems::Center, flex_direction: FlexDirection::Row, ..default() })
                .with_children(|row| {
                    spawn_labels(row);
                    spawn_plot(row);
                });
            chart.spawn((
                ExtrasText::Caption,
                Text::new(""),
                font(14.0),
                TextColor(DIM),
                Node { margin: UiRect::top(px(6)), ..default() },
            ));
        });
    panel
        .spawn((
            NoteBlock,
            Node { align_items: AlignItems::Center, margin: UiRect::top(px(16)), max_width: px(560), ..hidden },
        ))
        .with_children(|note| {
            note.spawn((ExtrasText::NoteTitle, Text::new(""), font(17.0), TextLayout::justify(Justify::Center)));
            note.spawn((
                ExtrasText::NoteBody,
                Text::new(""),
                font(15.0),
                TextColor(Color::srgba(1.0, 1.0, 1.0, 0.75)),
                TextLayout::justify(Justify::Center),
            ));
        });
}

/// The best and the worst value, at the top and bottom of the plot.
fn spawn_labels(row: &mut ChildSpawnerCommands) {
    row.spawn(Node {
        flex_direction: FlexDirection::Column,
        justify_content: JustifyContent::SpaceBetween,
        align_items: AlignItems::FlexEnd,
        height: px(PLOT_H),
        margin: UiRect::right(px(8)),
        ..default()
    })
    .with_children(|labels| {
        for which in [ExtrasText::Top, ExtrasText::Bottom] {
            labels.spawn((which, Text::new(""), font(14.0), TextColor(DIM), TextLayout::justify(Justify::Right)));
        }
    });
}

/// The plot: a line at the top and the bottom, and a dot for each slot.
fn spawn_plot(row: &mut ChildSpawnerCommands) {
    row.spawn(Node { width: px(PLOT_W), height: px(PLOT_H), position_type: PositionType::Relative, ..default() })
        .with_children(|plot| {
            for edge in [Node { top: px(0), ..default() }, Node { bottom: px(0), ..default() }] {
                plot.spawn((
                    Node { position_type: PositionType::Absolute, width: percent(100), height: px(1), ..edge },
                    BackgroundColor(GRID_LINE),
                ));
            }
            for slot in 0..CHART_RUNS {
                plot.spawn((
                    ChartDot(slot),
                    Node {
                        display: Display::None,
                        position_type: PositionType::Absolute,
                        border_radius: BorderRadius::MAX,
                        ..default()
                    },
                    BackgroundColor(EARLIER_DOT),
                ));
            }
        });
}

/// Fills the chart and the note from the finished run's extras, and hides them when
/// there is nothing to show or no room for them (see [`results::room`]).
pub(super) fn update(
    game: Res<Game>,
    window: Single<&Window>,
    panel: PanelTexts,
    mut blocks: Blocks,
    mut dots: Dots,
    mut texts: Query<(&ExtrasText, &mut Text)>,
) {
    if game.sim.phase() != Phase::Finished {
        return;
    }
    let extras = game.run.extras.as_ref();
    let lines = panel.iter().map(|t| t.0.lines().count()).sum();
    let has_chart = extras.is_some_and(|e| e.chart.is_some());
    let has_note = extras.is_some_and(|e| e.note.is_some());
    let (room_for_chart, room_for_note) = results::room(window.height(), lines, has_chart, has_note);
    let chart = extras.and_then(|e| e.chart.as_ref()).filter(|_| room_for_chart);
    let note = extras.and_then(|e| e.note.as_ref()).filter(|_| room_for_note);
    for (mut node, is_chart) in &mut blocks {
        let show = if is_chart { chart.is_some() } else { note.is_some() };
        set_display(&mut node, if show { Display::Flex } else { Display::None });
    }
    let values: Vec<f64> = chart.map(|c| c.points.iter().map(|p| p.value).collect()).unwrap_or_default();
    let higher_is_better = chart.is_some_and(|c| c.higher_is_better());
    place_dots(&results::dots(&values, higher_is_better), &mut dots);

    let knob = game.run.result.as_ref().and_then(|r| r.adaptive.as_ref()).map(|a| a.knob);
    let fmt = |v: f64| match knob {
        Some(k) => fmt_value(k, v as f32),
        None => format!("{v:.0}"),
    };
    let (top, bottom) = results::axis_labels(&values, higher_is_better, fmt);
    let adaptive = chart.is_some_and(|c| c.knob.is_some());
    for (which, mut text) in &mut texts {
        let new = match which {
            ExtrasText::Top => top.clone(),
            ExtrasText::Bottom => bottom.clone(),
            ExtrasText::Caption => results::caption(values.len(), adaptive),
            ExtrasText::NoteTitle => note.map(|n| results::game_text(&n.title)).unwrap_or_default(),
            ExtrasText::NoteBody => note.map(|n| results::game_text(&n.body)).unwrap_or_default(),
        };
        if text.0 != new {
            text.0 = new;
        }
    }
}

fn set_display(node: &mut Node, display: Display) {
    if node.display != display {
        node.display = display;
    }
}

/// Puts each slot's dot where `dots` says, and hides the slots past the last one.
fn place_dots(placed: &[results::Dot], dots: &mut Dots) {
    for (slot, mut node, mut background) in dots {
        let Some(dot) = placed.get(slot.0) else {
            set_display(&mut node, Display::None);
            continue;
        };
        let (left, top, size) = results::dot_box(dot);
        let mut new = node.clone();
        new.display = Display::Flex;
        new.left = px(left);
        new.top = px(top);
        new.width = px(size);
        new.height = px(size);
        node.set_if_neq(new);
        background.set_if_neq(BackgroundColor(if dot.this { THIS_DOT } else { EARLIER_DOT }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::IntoSystem;

    /// Bevy checks a system's queries for conflicts when it is set up, and a conflict
    /// panics when the trainer starts. This catches one without a window.
    #[test]
    fn the_results_systems_have_no_query_conflicts() {
        let mut world = World::new();
        let mut panel = IntoSystem::into_system(update);
        panel.initialize(&mut world);
        let mut overlay = IntoSystem::into_system(super::super::update_overlay);
        overlay.initialize(&mut world);
    }
}
