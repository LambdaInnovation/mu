use imgui_inspect::{InspectRenderDefault, InspectArgsDefault};
use crate::math::*;
use imgui::*;
use serde::export::PhantomData;
use strum::*;

pub struct VecDefaultInspect<T, U = T> where U: InspectRenderDefault<T> {
    marker: PhantomData<(T, U)>,
}

impl<T, TInspect> InspectRenderDefault<Vec<T>> for VecDefaultInspect<T, TInspect> where T: Default, TInspect: InspectRenderDefault<T> {
    fn render(_: &[&Vec<T>], _: &'static str, _: &Ui, _: &InspectArgsDefault) {
        unimplemented!();
    }

    fn render_mut(data: &mut [&mut Vec<T>], label: &'static str, ui: &Ui, _: &InspectArgsDefault) -> bool {
        assert_eq!(data.len(), 1, "Rendering >1 items not supported yet");
        let mut changed = false;
        TreeNode::new(&im_str!("{}", label)).build(ui, || {
            let v = &mut data[0];
            let mut indices_to_remove = vec![];
            for (i, item) in v.iter_mut().enumerate() {
                let mut keep = true;
                let show = CollapsingHeader::new(&im_str!("Item #{}", i))
                    .build_with_close_button(ui, &mut keep);
                if show {
                    let id = ui.push_id(Id::Int(i as i32));
                    changed |= TInspect::render_mut(&mut [item], "", ui,
                                         &InspectArgsDefault { header: Some(false), ..Default::default() });
                    id.pop(ui);
                }
                if !keep {
                    indices_to_remove.push(i);
                }
            }

            if indices_to_remove.len() > 0 {
                changed = true;
                for i in indices_to_remove.iter().rev() {
                    v.remove(*i);
                }
            }

            if ui.button(im_str!("Add"), [40., 20.]) {
                changed = true;
                v.push(T::default());
            }
            ui.same_line(0.);
            if ui.button(im_str!("Clear"), [40., 20.]) {
                changed = true;
                v.clear();
            }
        });

        changed
    }
}

pub struct Vec2DefaultInspect;

impl InspectRenderDefault<Vec2> for Vec2DefaultInspect {
    fn render(_: &[&Vec2], _: &'static str, _: &Ui, _: &InspectArgsDefault) {
        unimplemented!()
    }

    fn render_mut(data: &mut [&mut Vec2], label: &'static str, ui: &Ui, _args: &InspectArgsDefault) -> bool {
        assert_eq!(data.len(), 1, "Rendering >1 items not supported yet");

        let mut tmp_arr: [f32; 2] = (*data[0]).into();
        if ui.input_float2(&im_str!("{}", label), &mut tmp_arr).build() {
            *data[0] = vec2(tmp_arr[0], tmp_arr[1]);
            return true
        }

        false
    }
}

pub fn check_single_inspect<T>(data: &[&mut T]) {
    if data.len() != 1 {
        panic!("Multi inspect is not yet supported.");
    }
}

pub struct EnumComboInspect<T> {
    marker: PhantomData<T>
}

impl<T> InspectRenderDefault<T> for EnumComboInspect<T> where T: IntoEnumIterator + Into<&'static str> + PartialEq {
    fn render(_: &[&T], _: &'static str, _: &Ui, _: &InspectArgsDefault) {
        unimplemented!()
    }

    fn render_mut(data: &mut [&mut T], label: &'static str, ui: &Ui, _: &InspectArgsDefault) -> bool {
        check_single_inspect(data);

        let mut index = T::iter().enumerate()
            .find(|(_, x)| *x == *data[0])
            .unwrap().0;
        let names: Vec<_> = T::iter().map(|item| {
                let name: &'static str = item.into();
                im_str!("{}", name)
            })
            .collect();
        let names_ref: Vec<&ImStr> = names.iter()
            .map(|x| x.as_ref())
            .collect();

        if ComboBox::new(&im_str!("{}", label))
            .build_simple_string(ui, &mut index, &names_ref) {
            *data[0] = T::iter().nth(index).unwrap();

            true
        } else {
            false
        }
    }
}