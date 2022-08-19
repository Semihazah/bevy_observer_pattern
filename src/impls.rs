#[cfg(feature = "bevy_ui")]
mod ui {
    use bevy::{prelude::{Handle, Image}, ui::UiImage};

    use crate::Observer;

    impl Observer<String> for UiImage {
        fn receive_data(
            &mut self,
            data: &String,
            asset_server: &bevy::prelude::Res<bevy::prelude::AssetServer>,
            _sender: bevy::prelude::Entity,
        ) {
            self.0 = asset_server.load(data);
        }
    }

    impl Observer<Handle<Image>> for bevy::ui::UiImage {
        fn receive_data(
            &mut self,
            data: &Handle<Image>,
            _asset_server: &bevy::prelude::Res<bevy::prelude::AssetServer>,
            _sender: bevy::prelude::Entity,
        ) {
            self.0 = data.clone();
        }
    }
}
