use super::Tool;
use crate::gui::inputmap::{InputAction, InputMap};
use crate::gui::trainstation::{TrainTool, TrainToolKind};
use crate::rendering::immediate::ImmediateDraw;
use crate::uiworld::UiWorld;
use egregoria::vehicles::trains::wagons_positions;
use egregoria::Egregoria;
use geom::{Color, OBB};
use map_model::LaneKind;
use std::option::Option::None;

#[profiling::function]
pub fn addtrain(goria: &Egregoria, uiworld: &mut UiWorld) {
    let tool = *uiworld.read::<Tool>();
    if !matches!(tool, Tool::Train) {
        return;
    }

    uiworld.write_or_default::<TrainTool>();
    let res = uiworld.write::<TrainTool>();
    if !matches!(res.kind, TrainToolKind::AddTrain) {
        return;
    }

    let inp = uiworld.read::<InputMap>();

    let mut draw = uiworld.write::<ImmediateDraw>();
    let map = goria.map();
    let commands = &mut *uiworld.commands();

    let mpos = unwrap_ret!(inp.unprojected);

    let nearbylane = map.nearest_lane(mpos, LaneKind::Rail, Some(20.0));

    let nearbylane = match nearbylane.and_then(|x| map.lanes().get(x)) {
        Some(x) => x,
        None => {
            draw.circle(mpos, 10.0).color(common::config().gui_danger);
            return;
        }
    };

    let proj = nearbylane.points.project(mpos);
    let dist = nearbylane.points.length_at_proj(proj);

    let trainlength = 120.0;

    let mut drawtrain = |col: Color| {
        for (p, dir) in wagons_positions(&nearbylane.points, dist, 5) {
            draw.obb(OBB::new(p.xy(), dir.xy(), 16.5, 3.0), p.z + 0.5)
                .color(col);
        }
    };

    if dist <= trainlength {
        drawtrain(common::config().gui_danger);
        return;
    }

    drawtrain(common::config().gui_primary);

    if inp.just_act.contains(&InputAction::Select) {
        commands.add_train(dist, 5, nearbylane.id);
    }
}