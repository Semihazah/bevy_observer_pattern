#[cfg(feature = "bevy_ui")]
mod ui {
    use crate::Observer;

    impl Observer<String> for bevy::ui::UiImage {
        fn receive_data(
            &mut self,
            data: &String,
            asset_server: &bevy::prelude::Res<bevy::prelude::AssetServer>,
            _sender: bevy::prelude::Entity,
        ) {
            self.0 = asset_server.load(data);
        }
    }

    impl Observer<bevy::prelude::Handle<bevy::prelude::Image>> for bevy::ui::UiImage {
        fn receive_data(
            &mut self,
            data: &bevy::prelude::Handle<bevy::prelude::Image>,
            _asset_server: &bevy::prelude::Res<bevy::prelude::AssetServer>,
            _sender: bevy::prelude::Entity,
        ) {
            self.0 = data.clone();
        }
    }

    impl Observer<bevy::prelude::Color> for bevy::ui::UiColor {
        fn receive_data(&mut self, data: &bevy::prelude::Color, _asset_server: &bevy::prelude::Res<bevy::prelude::AssetServer>, _sender: bevy::prelude::Entity) {
            self.0 = data.clone();
        }
    }
}
