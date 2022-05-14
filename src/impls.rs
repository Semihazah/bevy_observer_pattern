use bevy::{ui::UiImage, text::{Text, TextSection}, prelude::{Component, FromWorld}, reflect::Reflect};

use crate::{ReceiveData, GiveData};

impl<T: bevy::prelude::Component + Send + Sync + 'static> ReceiveData<T> for T {
    fn receive_data<I: Into<T>>(&mut self, data: I, _reflect_data: &dyn bevy::reflect::Reflect, _asset_server: &bevy::prelude::Res<bevy::prelude::AssetServer>, _sender: bevy::prelude::Entity) {
        *self = data.into();
    }
}

impl<T: Component + FromWorld + Reflect + Clone> GiveData<T> for T {
    fn give_data(&self) -> T {
        self.clone()
    }
}

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