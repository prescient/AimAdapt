//! The chart on the results screen. What goes where is worked out in
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

/// One of the chart's dots, by its place from the left.
#[derive(Component)]
pub(super) struct ChartDot(usize);

/// The chart, to show or hide.
type Blocks<'w, 's> = Query<'w, 's, &'static mut Node, With<ChartBlock>>;

/// The chart's dots. Kept apart from the chart, which also has a `Node`.
type Dots<'w, 's> =
    Query<'w, 's, (&'static ChartDot, &'static mut Node, &'static mut BackgroundColor), Without<ChartBlock>>;

/// The results and the key hints, to count their lines. Kept apart from the texts
/// this module writes.
type PanelTexts<'w, 's> = Query<'w, 's, &'static Text, (Or<(With<OverlayText>, With<KeysText>)>, Without<ExtrasText>)>;

/// The text nodes that change.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(super) enum ExtrasText {
    Top,
    Bottom,
    Caption,
}

fn font(size: f32) -> TextFont {
    TextFont { font_size: FontSize::Px(size), ..default() }
}

/// Spawns the chart under the results panel, hidden.
pub(super) fn spawn(panel: &mut ChildSpawnerCommands) {
    let hidden = Node { display: Display::None, flex_direction: FlexDirection::Column, ..default() };
    panel
        .spawn((ChartBlock, Node { align_items: AlignItems::Center, margin: UiRect::top(px(16)), ..hidden }))
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

/// Fills the chart from the finished run's extras, and hides it when there is
/// nothing to show or no room for it (see [`results::room`]).
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
    let chart = extras.and_then(|e| e.chart.as_ref()).filter(|_| results::room(window.height(), lines, has_chart));
    for mut node in &mut blocks {
        set_display(&mut node, if chart.is_some() { Display::Flex } else { Display::None });
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
