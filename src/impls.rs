use bevy::ui::UiImage;

use crate::ReceiveData;

impl ReceiveData<String> for UiImage {
    fn recieve_data<I: Into<String>>(&mut self, data: I, _reflect_data: &dyn bevy::reflect::Reflect, asset_server: &bevy::prelude::Res<bevy::prelude::AssetServer>, _sender: bevy::prelude::Entity) {
        self.0 = asset_server.load(&data.into())
    }
}