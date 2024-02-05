use std::collections::HashSet;

use yakui::widgets::{CountGrid, List, Pad};
use yakui::{
    canvas, colored_box, constrained, use_state, Color, Constraints, CrossAxisAlignment,
    MainAxisAlignItems, MainAxisSize, Vec2,
};

use goryak::{
    constrained_viewport, mincolumn, minrow, on_primary_container, padxy, pady,
    selectable_label_primary, textc, Scrollable, Window,
};
use prototypes::{ItemID, DELTA_F64};
use simulation::economy::{
    EcoStats, ItemHistories, Market, HISTORY_SIZE, LEVEL_FREQS, LEVEL_NAMES,
};
use simulation::Simulation;

use crate::gui::GuiState;
use crate::uiworld::UiWorld;

#[derive(Copy, Clone, Default, PartialEq, Eq)]
pub enum EconomyTab {
    #[default]
    ImportExports,
    InternalTrade,
    MarketPrices,
}

#[derive(Copy, Clone, Default, PartialEq, Eq)]
pub enum HistoryType {
    #[default]
    Money,
    Items,
}

#[derive(Default)]
pub struct EconomyState {
    pub curlevel: usize,
    pub tab: EconomyTab,
    pub hist_type: HistoryType,
}

/// Economy window
/// Shows the economy stats
pub fn economy(uiw: &UiWorld, _: &Simulation) {
    uiw.window(
        Window {
            title: "Economy",
            pad: Pad::all(10.0),
            radius: 10.0,
        },
        |uiw| {
            uiw.write::<GuiState>().windows.economy_open = false;
        },
        |uiw, sim| {
            let mut state = uiw.write::<EconomyState>();
            let ecostats = sim.read::<EcoStats>();
            pady(10.0, || {
                let mut l = List::row();
                l.main_axis_size = MainAxisSize::Min;
                l.item_spacing = 10.0;
                l.show(|| {
                    let tabs = &[
                        ("Import/Exports", EconomyTab::ImportExports),
                        ("Internal Trade", EconomyTab::InternalTrade),
                        ("Market Prices", EconomyTab::MarketPrices),
                    ];

                    for (label, tab) in tabs {
                        if selectable_label_primary(state.tab == *tab, label).clicked {
                            state.tab = *tab;
                        }
                    }
                });
            });

            pady(10.0, || {
                let mut l = List::row();
                l.main_axis_size = MainAxisSize::Min;
                l.item_spacing = 10.0;
                l.show(|| {
                    for (i, level_name) in LEVEL_NAMES.iter().enumerate() {
                        if selectable_label_primary(state.curlevel == i, *level_name).clicked {
                            state.curlevel = i;
                        }
                    }

                    if state.tab == EconomyTab::ImportExports {
                        if selectable_label_primary(state.hist_type == HistoryType::Money, "Money")
                            .clicked
                        {
                            state.hist_type = HistoryType::Money;
                        }
                        if selectable_label_primary(state.hist_type == HistoryType::Items, "Items")
                            .clicked
                        {
                            state.hist_type = HistoryType::Items;
                        }
                    }
                });
            });
            let seconds_per_step = LEVEL_FREQS[state.curlevel] as f64 * DELTA_F64;
            let _xs: Vec<f64> = (0..HISTORY_SIZE)
                .map(|i| i as f64 * seconds_per_step)
                .collect();
            let EconomyState {
                curlevel,
                ref tab,
                hist_type,
            } = *state;

            let render_history = |history: &ItemHistories, hist_type: HistoryType| {
                padxy(5.0, 5.0, || {
                    mincolumn(|| {
                        let filterid = use_state(|| HashSet::<ItemID>::new());

                        pady(5.0, || {
                            colored_box(Color::BLACK, Vec2::new(300.0, 200.0)); // plot placeholder
                        });
                        canvas(|paint| {
                            //let tess = Tesselator::new(
                            //    Some(AABB::new((0.0, 0.0).into(), (300.0, 200.0).into())),
                            //    1.0,
                            //);
                        });

                        let scroll_constrained = Constraints {
                            min: Vec2::new(300.0, 0.0),
                            max: Vec2::new(300.0, 300.0),
                        };

                        constrained(scroll_constrained, || {
                            let mut s = Scrollable::vertical();
                            s.main_axis_size = MainAxisSize::Min;
                            s.show(|| {
                                constrained(
                                    Constraints::loose(Vec2::new(100000.0, 1000000.0)),
                                    || {
                                        let mut overall_total = 0;
                                        let mut g = CountGrid::col(2);
                                        g.cross_axis_alignment = CrossAxisAlignment::Stretch;
                                        g.main_axis_size = MainAxisSize::Min;
                                        g.main_axis_align_items = MainAxisAlignItems::Center;
                                        g.show(|| {
                                            let mut histories: Vec<_> = history
                                                .iter_histories(curlevel)
                                                .map(|(id, level)| {
                                                    (id, {
                                                        match hist_type {
                                                            HistoryType::Items => level
                                                                .past_ring_items
                                                                .iter()
                                                                .sum::<i64>(),
                                                            HistoryType::Money => level
                                                                .past_ring_money
                                                                .iter()
                                                                .map(|x| x.bucks().abs())
                                                                .sum::<i64>(),
                                                        }
                                                    })
                                                })
                                                .filter(|(_, x)| *x != 0)
                                                .collect();
                                            histories.sort_by_key(|(_, sum)| *sum);

                                            for (id, sum) in histories {
                                                let enabled = filterid.borrow().contains(&id);

                                                minrow(|| {
                                                    if selectable_label_primary(
                                                        enabled,
                                                        &id.prototype().name,
                                                    )
                                                    .clicked
                                                    {
                                                        if !enabled {
                                                            filterid.borrow_mut().insert(id);
                                                        } else {
                                                            filterid.borrow_mut().remove(&id);
                                                        }
                                                    }
                                                });
                                                let suffix = match hist_type {
                                                    HistoryType::Items => "",
                                                    HistoryType::Money => "$",
                                                };
                                                padxy(5.0, 5.0, || {
                                                    textc(
                                                        on_primary_container(),
                                                        format!("{}{}", sum, suffix),
                                                    );
                                                });
                                                overall_total += sum;
                                            }
                                            if matches!(hist_type, HistoryType::Money) {
                                                textc(
                                                    on_primary_container(),
                                                    format!("Total: {}$", overall_total),
                                                );
                                            }
                                        });
                                    },
                                );
                            });
                        });
                    });
                });
            };

            match tab {
                EconomyTab::ImportExports => {
                    let (label_left, label_right) = match hist_type {
                        HistoryType::Items => ("Imports", "Exports"),
                        HistoryType::Money => ("Expenses", "Income"),
                    };

                    constrained_viewport(|| {
                        let mut grid = CountGrid::col(2);
                        grid.main_axis_size = MainAxisSize::Min;
                        grid.show(|| {
                            padxy(5.0, 5.0, || textc(on_primary_container(), label_left));
                            padxy(5.0, 5.0, || textc(on_primary_container(), label_right));

                            render_history(&ecostats.imports, hist_type);
                            render_history(&ecostats.exports, hist_type);
                        });
                    });
                }
                EconomyTab::InternalTrade => {
                    render_history(&ecostats.internal_trade, HistoryType::Items);
                }
                EconomyTab::MarketPrices => {
                    render_market_prices(sim);
                }
            }
        },
    );
}

/*
let render_history = |ui: &mut Ui, history: &ItemHistories, hist_type: HistoryType| {
    egui_plot::Plot::new("ecoplot")
        .height(200.0)
        .allow_boxed_zoom(false)
        .include_y(0.0)
        .include_x(0.0)
        .allow_drag(false)
        .allow_scroll(false)
        .allow_zoom(false)
        .show(ui, |ui| {
            let mut overallmax = 0;
            let cursor = history.cursors()[curlevel];
            for (id, history) in history.iter_histories(curlevel) {
                if !filter.is_empty() && !filter.contains(&id) {
                    continue;
                }
                let holder;
                let ring = match hist_type {
                    HistoryType::Items => &history.past_ring_items,
                    HistoryType::Money => {
                        holder = history.past_ring_money.map(|x| x.bucks().abs());
                        &holder
                    }
                };

                let maxval = *ring.iter().max().unwrap();
                if maxval == 0 {
                    continue;
                }
                if maxval > overallmax {
                    overallmax = maxval;
                }

                let h = id.hash();
                let random_col = Color::new(
                    0.5 + 0.5 * common::rand::rand2(h as f32, 0.0),
                    0.5 + 0.5 * common::rand::rand2(h as f32, 1.0),
                    0.5 + 0.5 * common::rand::rand2(h as f32, 2.0),
                    1.0,
                );

                let c_next = (cursor + 1) % HISTORY_SIZE;

                let mut first_zeros = false;
                let heights = ring[c_next..HISTORY_SIZE]
                    .iter()
                    .chain(ring[0..c_next].iter())
                    .copied()
                    .zip(xs.iter())
                    .map(|(v, x)| [*x, v as f64])
                    .filter(|[_, y]| {
                        if !first_zeros && *y > 0.0 {
                            first_zeros = true;
                        }
                        first_zeros
                    });

                ui.line(
                    Line::new(PlotPoints::from_iter(heights))
                        .color(Color32::from_rgba_unmultiplied(
                            (random_col.r * 255.0) as u8,
                            (random_col.g * 255.0) as u8,
                            (random_col.b * 255.0) as u8,
                            (random_col.a * 255.0) as u8,
                        ))
                        .name(&id.prototype().name),
                );
            }
            ui.line(
                Line::new(
                    [
                        [0.0, 0.0],
                        [
                            HISTORY_SIZE as f64 * seconds_per_step,
                            1.0 + 1.25 * overallmax as f64,
                        ],
                    ]
                    .into_iter()
                    .collect::<PlotPoints>(),
                )
                .width(0.001)
                .color(Color32::from_white_alpha(1)),
            );
        });
};
*/

fn render_market_prices(sim: &Simulation) {
    let market = sim.read::<Market>();

    let mut grid = CountGrid::col(2);
    grid.main_axis_size = MainAxisSize::Min;
    grid.show(|| {
        for (id, market) in market.iter() {
            padxy(5.0, 3.0, || {
                textc(on_primary_container(), &id.prototype().name)
            });
            padxy(5.0, 3.0, || {
                textc(on_primary_container(), market.ext_value.to_string())
            });
        }
    });
}
