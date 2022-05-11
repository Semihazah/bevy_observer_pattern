use bevy::{ui::UiImage, text::{Text, TextSection}};

use crate::ReceiveData;

impl ReceiveData<String> for UiImage {
    fn receive_data<I: Into<String>>(&mut self, data: I, _reflect_data: &dyn bevy::reflect::Reflect, asset_server: &bevy::prelude::Res<bevy::prelude::AssetServer>, _sender: bevy::prelude::Entity) {
        self.0 = asset_server.load(&data.into())
    }
}

impl ReceiveData<String> for Text {
    fn receive_data<I: Into<String>>(&mut self, data: I, _reflect_data: &dyn bevy::reflect::Reflect, _asset_server: &bevy::prelude::Res<bevy::prelude::AssetServer>, _sender: bevy::prelude::Entity) {
        let mut section = match self.sections.get_mut(0) {
            Some(s) => s,
            None => {
                self.sections.push(TextSection::default());
                self.sections.get_mut(0).unwrap()
            }
        };
        section.value = data.into();
    }
}