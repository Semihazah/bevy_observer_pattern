use std::{
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

use bevy::{
    app::{App, CoreStage},
    asset::AssetServer,
    ecs::{
        component::Component,
        entity::Entity,
        entity::{EntityMap, MapEntities, MapEntitiesError},
        query::{Changed, QueryEntityError},
        reflect::{ReflectComponent, ReflectMapEntities},
        schedule::ParallelSystemDescriptorCoercion,
        system::{Command, EntityCommands, Query, Res, SystemState},
        world::{EntityMut, World},
    },
    reflect::{FromReflect, Reflect},
    utils::HashSet,
};

mod impls;

// Implementation of the observer pattern between components on entities.
// Observers will be given a reference to the subject, a reference to the asset server,
// and the subject's entity.
// Register every subject type with register_subject when building app.
// Register every observer AND the subject type with register_observer when building app.
// Call set_observer when building entity to mark as an observer to another entity.

/// An observer component. Mutated subjects will update this component.
pub trait Observer<T: Send + Sync + 'static>: Component {
    fn receive_data(&mut self, data: &T, asset_server: &Res<AssetServer>, sender: Entity);
}

/// Marks a component as a possible Subject that can give T
/// All components automatically implement this for T = Self
pub trait Subject<T: Send + Sync + 'static>: Component {
    fn give_data(&self) -> &T;
}

impl<T: Component> Subject<T> for T {
    fn give_data(&self) -> &T {
        self
    }
}

/// List of entities that are observing this entity.
#[derive(Reflect, FromReflect, Clone, Component)]
#[reflect(Component, MapEntities)]
pub struct ObserverList<T: Send + Sync + 'static, S: Subject<T>, O: Observer<T>> {
    observers: HashSet<Entity>,

    #[reflect(ignore)]
    phantom_data: PhantomData<T>,

    #[reflect(ignore)]
    phantom_subject: PhantomData<S>,

    #[reflect(ignore)]
    phantom_observer: PhantomData<O>,
}

impl<T: Send + Sync + 'static, S: Subject<T>, O: Observer<T>> Deref for ObserverList<T, S, O> {
    type Target = HashSet<Entity>;
    fn deref(&self) -> &Self::Target {
        &self.observers
    }
}

impl<T: Send + Sync + 'static, S: Subject<T>, O: Observer<T>> DerefMut for ObserverList<T, S, O> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.observers
    }
}

impl<T: Send + Sync + 'static, S: Subject<T>, O: Observer<T>> ObserverList<T, S, O> {
    pub fn new(list: impl IntoIterator<Item = Entity>) -> Self {
        ObserverList {
            observers: list.into_iter().collect(),
            phantom_data: PhantomData,
            phantom_subject: PhantomData,
            phantom_observer: PhantomData,
        }
    }
}
impl<T: Send + Sync + 'static, S: Subject<T>, O: Observer<T>> Default for ObserverList<T, S, O> {
    fn default() -> Self {
        ObserverList::new(vec![])
    }
}
impl<T: Send + Sync + 'static, S: Subject<T>, O: Observer<T>> MapEntities
    for ObserverList<T, S, O>
{
    fn map_entities(&mut self, m: &EntityMap) -> Result<(), MapEntitiesError> {
        let mut new_set = HashSet::default();
        for receiver in self.observers.iter() {
            new_set.insert(m.get(*receiver).unwrap());
        }
        self.observers = new_set;
        Ok(())
    }
}

struct ObserverBuildCommand<T: Send + Sync + 'static, S: Subject<T>, O: Observer<T>> {
    pub observer: Entity,
    pub subjects: Vec<Entity>,
    phantom_data: PhantomData<T>,
    phantom_subject: PhantomData<S>,
    phantom_observer: PhantomData<O>,
}

impl<T: Send + Sync + 'static, S: Subject<T>, O: Observer<T>> Command
    for ObserverBuildCommand<T, S, O>
{
    fn write(self, world: &mut World) {
        for &source in self.subjects.iter() {
            match world.entity(source).contains::<ObserverList<T, S, O>>() {
                false => {
                    world
                        .entity_mut(source)
                        .insert(ObserverList::<T, S, O>::new(vec![self.observer]));
                }
                true => {
                    let mut entity_mut = world.entity_mut(source);
                    let mut observer_list = entity_mut.get_mut::<ObserverList<T, S, O>>().unwrap();
                    observer_list.observers.insert(self.observer);
                }
            }
        }

        let mut system_state: SystemState<(Res<AssetServer>, Query<&mut O>, Query<(Entity, &S)>)> =
            SystemState::new(world);

        let (asset_server, mut observer_query, subject_query) = system_state.get_mut(world);

        if let Ok(mut observer) = observer_query.get_mut(self.observer) {
            for &source in self.subjects.iter() {
                if let Ok((subject, subject_comp)) = subject_query.get(source) {
                    let data = subject_comp.give_data();
                    observer.receive_data(data, &asset_server, subject)
                }
            }
        }
    }
}

pub trait ObserverBuildCommandExt {
    /// Sets the component O on this entity to observe component S on the source entities.
    fn set_observer<T: Send + Sync + 'static, S: Subject<T>, O: Observer<T>>(
        &mut self,
        source: Vec<Entity>,
    ) -> &mut Self;
}

impl<'w, 's, 'a> ObserverBuildCommandExt for EntityCommands<'w, 's, 'a> {
    fn set_observer<T: Send + Sync + 'static, S: Subject<T>, O: Observer<T>>(
        &mut self,
        sources: Vec<Entity>,
    ) -> &mut Self {
        let id = self.id();

        self.commands().add(ObserverBuildCommand::<T, S, O> {
            observer: id,
            subjects: sources,
            phantom_data: PhantomData,
            phantom_subject: PhantomData,
            phantom_observer: PhantomData,
        });

        self
    }
}

impl<'w> ObserverBuildCommandExt for EntityMut<'w> {
    fn set_observer<T: Send + Sync + 'static, S: Subject<T>, O: Observer<T>>(
        &mut self,
        sources: Vec<Entity>,
    ) -> &mut Self {
        let id = self.id();
        unsafe {
            let world = self.world_mut();
            ObserverBuildCommand::<T, S, O> {
                observer: id,
                subjects: sources,
                phantom_data: PhantomData,
                phantom_subject: PhantomData,
                phantom_observer: PhantomData,
            }
            .write(world)
        }

        self
    }
}

/// Receives subject events from subjects and updates any observer component in ObserverList.
fn recieve_subject_event<T: Send + Sync + 'static, S: Subject<T>, O: Observer<T>>(
    asset_server: Res<AssetServer>,
    mut observer_query: Query<&mut O>,
    mut observer_list_query: Query<(Entity, &S, &mut ObserverList<T, S, O>), Changed<S>>,
) {
    for (subject, subject_comp, mut observer_list) in observer_list_query.iter_mut() {
        let data = Subject::<T>::give_data(subject_comp);
        let mut remove_list = Vec::<Entity>::new();
        for &observer in observer_list.observers.iter() {
            match observer_query.get_mut(observer) {
                Ok(mut observer) => {
                    observer.receive_data(data, &asset_server, subject);
                }
                Err(QueryEntityError::NoSuchEntity { .. }) => remove_list.push(observer),
                _ => (),
            }
        }

        observer_list.observers.retain(|x| !remove_list.contains(x));
    }
}

pub trait ObserverRegisterExt {
    /// Register a type as capable of observing.
    fn register_observer<T: Send + Sync + 'static, S: Subject<T>, O: Observer<T>>(
        &mut self,
    ) -> &mut Self;
}

impl ObserverRegisterExt for App {
    fn register_observer<T: Send + Sync + 'static, S: Subject<T>, O: Observer<T>>(
        &mut self,
    ) -> &mut Self {
        self.register_type::<ObserverList<T, S, O>>()
            .add_system_to_stage(
                CoreStage::PostUpdate,
                recieve_subject_event::<T, S, O>.after("SubjectUpdate"),
            );
        self
    }
}

#[cfg(test)]
mod tests {
    use bevy::{asset::create_platform_default_asset_io, prelude::*, tasks::TaskPool};

    use crate::{Observer, ObserverBuildCommandExt, ObserverRegisterExt, Subject};

    #[derive(Component)]
    struct TestSubject {
        a: String,
        b: u32,
    }

    impl Subject<String> for TestSubject {
        fn give_data(&self) -> &String {
            &self.a
        }
    }

    #[derive(Component, Default)]
    struct TestObserver {
        a: Option<String>,
        b: Option<u32>,
    }

    impl Observer<String> for TestObserver {
        fn receive_data(
            &mut self,
            data: &String,
            _asset_server: &Res<AssetServer>,
            _sender: Entity,
        ) {
            self.a = Some(data.clone());
        }
    }

    impl Observer<TestSubject> for TestObserver {
        fn receive_data(
            &mut self,
            data: &TestSubject,
            _asset_server: &Res<AssetServer>,
            _sender: Entity,
        ) {
            self.a = Some(data.a.clone());
            self.b = Some(data.b.clone());
        }
    }

    fn mutate_data(mut query: Query<&mut TestSubject, Added<TestSubject>>) {
        for mut giver in query.iter_mut() {
            giver.a = "Farewell World!".to_string();
            giver.b = 12;
        }
    }

    /// Quick test to see if mutations are picked up.
    /// Subject and Observer are registered
    /// mutate_data changes the subject to something else
    /// Subject and Observer entities are spawned
    /// After one frame, check and see if the values match.
    #[test]
    fn test_data_sync() {
        let mut app = App::new();

        let source = create_platform_default_asset_io(&mut app);
        let asset_server = AssetServer::with_boxed_io(source, TaskPool::new());

        app.insert_resource(asset_server)
            .register_observer::<String, TestSubject, TestObserver>()
            .add_system(mutate_data);

        let g = app
            .world
            .spawn()
            .insert(TestSubject {
                a: "Hello World!".to_string(),
                b: 42,
            })
            .id();

        let r = app
            .world
            .spawn()
            .insert(TestObserver::default())
            .set_observer::<String, TestSubject, TestObserver>(vec![g])
            .id();

        app.update();

        assert_eq!(
            app.world.get::<TestObserver>(r).unwrap().a,
            Some("Farewell World!".to_string())
        );
    }

    /// Same as above, but testing giving the entire component
    #[test]
    fn test_self_data_sync() {
        let mut app = App::new();

        let source = create_platform_default_asset_io(&mut app);
        let asset_server = AssetServer::with_boxed_io(source, TaskPool::new());

        app.insert_resource(asset_server)
            .register_observer::<TestSubject, TestSubject, TestObserver>()
            .add_system(mutate_data);

        let g = app
            .world
            .spawn()
            .insert(TestSubject {
                a: "Hello World!".to_string(),
                b: 42,
            })
            .id();

        let r = app
            .world
            .spawn()
            .insert(TestObserver::default())
            .set_observer::<TestSubject, TestSubject, TestObserver>(vec![g])
            .id();

        app.update();

        assert_eq!(
            app.world.get::<TestObserver>(r).unwrap().a,
            Some("Farewell World!".to_string())
        );
        assert_eq!(app.world.get::<TestObserver>(r).unwrap().b, Some(12));
    }
}
