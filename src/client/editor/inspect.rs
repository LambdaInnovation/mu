use imgui_inspect::{InspectRenderDefault, InspectArgsDefault};
use crate::math::*;
use imgui::*;
use serde::export::PhantomData;

pub struct VecDefaultInspect<T, U = T> where U: InspectRenderDefault<T> {
    marker: PhantomData<(T, U)>,
}

impl<T, TInspect> InspectRenderDefault<Vec<T>> for VecDefaultInspect<T, TInspect> where T: Default, TInspect: InspectRenderDefault<T> {
    fn render(data: &[&Vec<T>], label: &'static str, ui: &Ui, args: &InspectArgsDefault) {
        ui.text(im_str!("TODO: Vector"));
    }

    fn render_mut(data: &mut [&mut Vec<T>], label: &'static str, ui: &Ui, args: &InspectArgsDefault) -> bool {
        assert_eq!(data.len(), 1, "Rendering >1 items not supported yet");
        TreeNode::new(&im_str!("{}", label)).build(ui, || {
            let v = &mut data[0];
            for (i, item) in v.iter_mut().enumerate() {
                let mut keep = true;
                let show = CollapsingHeader::new(&im_str!("Item #{}", i))
                    .build_with_close_button(ui, &mut keep);
                if show {
                    TInspect::render_mut(&mut [item], "", ui,
                                         &InspectArgsDefault { header: Some(false), ..Default::default() });
                }
            }
        });

        true
    }
}

pub struct Vec2DefaultInspect;

impl InspectRenderDefault<Vec2> for Vec2DefaultInspect {
    fn render(data: &[&Vec2], label: &'static str, ui: &Ui, args: &InspectArgsDefault) {
        assert_eq!(data.len(), 1, "Rendering >1 items not supported yet");

        let mut tmp_arr: [f32; 2] = (*data[0]).into();
        ui.input_float2(&im_str!("{}", label), &mut tmp_arr).build();
    }

    fn render_mut(data: &mut [&mut Vec2], label: &'static str, ui: &Ui, args: &InspectArgsDefault) -> bool {
        assert_eq!(data.len(), 1, "Rendering >1 items not supported yet");

        let mut tmp_arr: [f32; 2] = (*data[0]).into();
        ui.input_float2(&im_str!("{}", label), &mut tmp_arr).build();
        *data[0] = vec2(tmp_arr[0], tmp_arr[1]);

        true
    }
}