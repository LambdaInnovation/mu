use imgui_inspect::{InspectRenderDefault, InspectArgsDefault};
use crate::math::*;
use imgui::*;
use serde::export::PhantomData;

pub struct VecDefaultInspect<T, U = T> where U: InspectRenderDefault<T> {
    marker: PhantomData<U>,
    marker2: PhantomData<T>
}


impl<T, TInspect> InspectRenderDefault<Vec<T>> for VecDefaultInspect<T, TInspect> where TInspect: InspectRenderDefault<T> {
    fn render(data: &[&Vec<T>], label: &'static str, ui: &Ui, args: &InspectArgsDefault) {
        ui.text(im_str!("TODO: Vector"));
    }

    fn render_mut(data: &mut [&mut Vec<T>], label: &'static str, ui: &Ui, args: &InspectArgsDefault) -> bool {
        ui.text(im_str!("TODO: Vector"));
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