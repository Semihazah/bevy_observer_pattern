use std::marker::PhantomData;

use bevy::{
    app::{App, CoreStage},
    asset::AssetServer,
    ecs::{
        component::Component,
        entity::Entity,
        entity::{EntityMap, MapEntities, MapEntitiesError},
        query::{Changed, Or},
        reflect::{ReflectComponent, ReflectMapEntities},
        schedule::ParallelSystemDescriptorCoercion,
        system::{Command, EntityCommands, Query, Res},
        world::{EntityMut, FromWorld, World},
    },
    reflect::{FromReflect, Reflect},
};

#[derive(Reflect, FromReflect, Clone, Component, Default)]
#[reflect(Component)]
pub struct SyncData<
    T: Default + Send + Sync + 'static,
    G: GiveData<T> + Default,
    R: RecieveData<T> + Default,
> {
    pub sources: Vec<Entity>,

    #[reflect(ignore)]
    phantom_data: PhantomData<T>,

    #[reflect(ignore)]
    phantom_giver: PhantomData<G>,

    #[reflect(ignore)]
    phantom_reciever: PhantomData<R>,
}

impl<T: Default + Send + Sync + 'static, G: GiveData<T> + Default, R: RecieveData<T> + Default>
    SyncData<T, G, R>
{
    pub fn new(sources: Vec<Entity>) -> Self {
        SyncData {
            sources,
            phantom_data: PhantomData,
            phantom_giver: PhantomData,
            phantom_reciever: PhantomData,
        }
    }
}

impl<T: Default + Send + Sync + 'static, G: GiveData<T> + Default, R: RecieveData<T> + Default>
    MapEntities for SyncData<T, G, R>
{
    fn map_entities(&mut self, m: &EntityMap) -> Result<(), MapEntitiesError> {
        for source in self.sources.iter_mut() {
            *source = m.get(*source).unwrap();
        }

        Ok(())
    }
}

pub trait RecieveData<T: Send + Sync + 'static>: Component {
    fn recieve_data<I: Into<T>>(
        &mut self,
        data: I,
        reflect_data: &dyn Reflect,
        asset_server: &Res<AssetServer>,
        sender: Entity,
    );
}

pub trait GiveData<T: Send + Sync + 'static>: Component + FromWorld + Reflect {
    fn give_data(&self) -> T;
}

#[derive(Reflect, FromReflect, Clone, Component)]
#[reflect(Component, MapEntities)]
pub struct GiveList<T: Send + Sync + 'static, G: GiveData<T>, R: RecieveData<T>> {
    pub recievers: Vec<Entity>,

    #[reflect(ignore)]
    phantom_data: PhantomData<T>,

    #[reflect(ignore)]
    phantom_giver: PhantomData<G>,

    #[reflect(ignore)]
    phantom_reciever: PhantomData<R>,
}

impl<T: Send + Sync + 'static, G: GiveData<T>, R: RecieveData<T>> GiveList<T, G, R> {
    pub fn new(list: Vec<Entity>) -> Self {
        GiveList {
            recievers: list,
            phantom_data: PhantomData,
            phantom_giver: PhantomData,
            phantom_reciever: PhantomData,
        }
    }
}
impl<T: Send + Sync + 'static, G: GiveData<T>, R: RecieveData<T>> Default for GiveList<T, G, R> {
    fn default() -> Self {
        GiveList {
            recievers: Vec::default(),
            phantom_data: PhantomData,
            phantom_giver: PhantomData,
            phantom_reciever: PhantomData,
        }
    }
}
impl<T: Send + Sync + 'static, G: GiveData<T>, R: RecieveData<T>> MapEntities
    for GiveList<T, G, R>
{
    fn map_entities(&mut self, m: &EntityMap) -> Result<(), MapEntitiesError> {
        for reciever in self.recievers.iter_mut() {
            *reciever = m.get(*reciever).unwrap();
        }

        Ok(())
    }
}

pub struct SyncToDataCommand<
    T: Send + Sync + 'static,
    G: GiveData<T> + Default,
    R: RecieveData<T> + Default,
> {
    pub entity: Entity,
    pub sources: Vec<Entity>,
    phantom_data: PhantomData<T>,
    phantom_giver: PhantomData<G>,
    phantom_reciever: PhantomData<R>,
}

impl<T: Default + Send + Sync + 'static, G: GiveData<T> + Default, R: RecieveData<T> + Default>
    Command for SyncToDataCommand<T, G, R>
{
    fn write(self, world: &mut World) {
        match world.entity_mut(self.entity).get_mut::<SyncData<T, G, R>>() {
            Some(mut sync_list) => sync_list.sources.append(&mut self.sources.clone()),
            None => {
                world
                    .entity_mut(self.entity)
                    .insert(SyncData::<T, G, R>::new(self.sources.clone()));
            }
        }

        for source in self.sources {
            match world.entity(source).contains::<GiveList<T, G, R>>() {
                false => {
                    world
                        .entity_mut(source)
                        .insert(GiveList::<T, G, R>::new(vec![self.entity]));
                }
                true => {
                    let mut entity_mut = world.entity_mut(source);
                    let mut give_list = entity_mut.get_mut::<GiveList<T, G, R>>().unwrap();
                    give_list.recievers.push(self.entity);
                }
            }
        }
    }
}

pub trait SyncToDataCommandExt {
    fn sync_to_data<
        T: Default + Send + Sync + 'static,
        G: GiveData<T> + Default,
        R: RecieveData<T> + Default,
    >(
        &mut self,
        source: Vec<Entity>,
    ) -> &mut Self;
}

impl<'w, 's, 'a> SyncToDataCommandExt for EntityCommands<'w, 's, 'a> {
    fn sync_to_data<
        T: Default + Send + Sync + 'static,
        G: GiveData<T> + Default,
        R: RecieveData<T> + Default,
    >(
        &mut self,
        sources: Vec<Entity>,
    ) -> &mut Self {
        let id = self.id();

        self.commands().add(SyncToDataCommand::<T, G, R> {
            entity: id,
            sources,
            phantom_data: PhantomData,
            phantom_giver: PhantomData,
            phantom_reciever: PhantomData,
        });

        self
    }
}

impl<'w> SyncToDataCommandExt for EntityMut<'w> {
    fn sync_to_data<
        T: Default + Send + Sync + 'static,
        G: GiveData<T> + Default,
        R: RecieveData<T> + Default,
    >(
        &mut self,
        sources: Vec<Entity>,
    ) -> &mut Self {
        let id = self.id();
        unsafe {
            let world = self.world_mut();
            SyncToDataCommand::<T, G, R> {
                entity: id,
                sources,
                phantom_data: PhantomData,
                phantom_giver: PhantomData,
                phantom_reciever: PhantomData,
            }
            .write(world)
        }

        self
    }
}

pub fn sync_data<T: Send + Sync + 'static, G: GiveData<T>, R: RecieveData<T>>(
    asset_server: Res<AssetServer>,
    mut give_query: Query<
        (Entity, &G, &mut GiveList<T, G, R>),
        Or<(Changed<G>, Changed<GiveList<T, G, R>>)>,
    >,
    mut recieve_query: Query<&mut R>,
) {
    for (sender, data, mut list) in give_query.iter_mut() {
        let mut remove_list = Vec::new();
        for recieve_entity in list.recievers.iter() {
            //println!("Syncing changed data for types {}, {}, {}, reciever = {:?}", type_name::<T>(), type_name::<G>(), type_name::<R>(), recieve_entity);
            if let Ok(mut reciever) = recieve_query.get_mut(*recieve_entity) {
                //println!("Sync data success!");
                reciever.recieve_data(
                    data.give_data(),
                    data as &dyn Reflect,
                    &asset_server,
                    sender,
                );
            } else {
                //println!("Sync data failed! Could not find reciever!");

                remove_list.push(*recieve_entity);
            }
        }

        list.recievers
            .retain(|entity| !remove_list.contains(entity))
    }
}

pub fn sync_init_data<
    T: Default + Send + Sync + 'static,
    G: GiveData<T> + Default,
    R: RecieveData<T> + Default,
>(
    asset_server: Res<AssetServer>,
    mut recieve_query: Query<(&mut R, &SyncData<T, G, R>), Changed<SyncData<T, G, R>>>,
    give_query: Query<&G>,
) {
    for (mut reciever, sync) in recieve_query.iter_mut() {
        //println!("Syncing init data for types {}, {}, {}", type_name::<T>(), type_name::<G>(), type_name::<R>());
        for source in sync.sources.iter() {
            if let Ok(giver) = give_query.get(*source) {
                //println!("Found giver!");
                reciever.recieve_data(
                    giver.give_data(),
                    giver as &dyn Reflect,
                    &asset_server,
                    *source,
                );
            }
        }
    }
}
pub trait SyncBuilder {
    fn register_data_sync<T, G, R>(&mut self) -> &mut Self
    where
        T: Default + Send + Sync + 'static,
        G: GiveData<T> + Default,
        R: RecieveData<T> + Default;
}

impl SyncBuilder for App {
    fn register_data_sync<T, G, R>(&mut self) -> &mut Self
    where
        T: Default + Send + Sync + 'static,
        G: GiveData<T> + Default,
        R: RecieveData<T> + Default,
    {
        self.register_type::<SyncData<T, G, R>>()
            .register_type::<GiveList<T, G, R>>()
            .add_system_to_stage(
                CoreStage::PostUpdate,
                sync_data::<T, G, R>.label("sync_data"),
            )
            .add_system_to_stage(
                CoreStage::PostUpdate,
                sync_init_data::<T, G, R>.label("sync_data"),
            );

        self
    }
}
