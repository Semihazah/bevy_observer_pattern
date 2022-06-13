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
    prelude::{EventReader, EventWriter},
    reflect::{FromReflect, Reflect},
};

// Implementation of the observer pattern between components on entities.
// Observers will be given a reference to the subject, a reference to the asset server,
// and the subject's entity.
// Register every subject type with register_subject when building app.
// Register every observer AND the subject type with register_observer when building app.
// Call set_observer when building entity to mark as an observer to another entity.

/// An observer component. Mutated subjects will update this component.
pub trait Observer<S: Component>: Component {
    fn receive_data(&mut self, data: &S, asset_server: &Res<AssetServer>, sender: Entity);
}

/// List of entities that are observing this entity.
#[derive(Reflect, FromReflect, Clone, Component)]
#[reflect(Component, MapEntities)]
pub struct ObserverList<S: Component, O: Observer<S>> {
    observers: Vec<Entity>,

    #[reflect(ignore)]
    phantom_giver: PhantomData<S>,

    #[reflect(ignore)]
    phantom_receiver: PhantomData<O>,
}

impl<S: Component, O: Observer<S>> Deref for ObserverList<S, O> {
    type Target = Vec<Entity>;
    fn deref(&self) -> &Self::Target {
        &self.observers
    }
}

impl<S: Component, O: Observer<S>> DerefMut for ObserverList<S, O> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.observers
    }
}

impl<S: Component, O: Observer<S>> ObserverList<S, O> {
    pub fn new(list: Vec<Entity>) -> Self {
        ObserverList {
            observers: list,
            phantom_giver: PhantomData,
            phantom_receiver: PhantomData,
        }
    }
}
impl<S: Component, O: Observer<S>> Default for ObserverList<S, O> {
    fn default() -> Self {
        ObserverList {
            observers: Vec::default(),
            phantom_giver: PhantomData,
            phantom_receiver: PhantomData,
        }
    }
}
impl<S: Component, O: Observer<S>> MapEntities for ObserverList<S, O> {
    fn map_entities(&mut self, m: &EntityMap) -> Result<(), MapEntitiesError> {
        for receiver in self.observers.iter_mut() {
            *receiver = m.get(*receiver).unwrap();
        }

        Ok(())
    }
}

pub struct SyncToDataCommand<S: Component, O: Observer<S>> {
    pub observer: Entity,
    pub subjects: Vec<Entity>,
    phantom_subject: PhantomData<S>,
    phantom_observer: PhantomData<O>,
}

impl<S: Component, O: Observer<S>> Command for SyncToDataCommand<S, O> {
    fn write(self, world: &mut World) {
        for &source in self.subjects.iter() {
            match world.entity(source).contains::<ObserverList<S, O>>() {
                false => {
                    world
                        .entity_mut(source)
                        .insert(ObserverList::<S, O>::new(vec![self.observer]));
                }
                true => {
                    let mut entity_mut = world.entity_mut(source);
                    let mut observer_list = entity_mut.get_mut::<ObserverList<S, O>>().unwrap();
                    observer_list.observers.push(self.observer);
                }
            }
        }

        let mut system_state: SystemState<(Res<AssetServer>, Query<&mut O>, Query<(Entity, &S)>)> =
            SystemState::new(world);

        let (asset_server, mut observer_query, subject_query) = system_state.get_mut(world);

        if let Ok(mut observer) = observer_query.get_mut(self.observer) {
            for &source in self.subjects.iter() {
                if let Ok((subject, subject_comp)) = subject_query.get(source) {
                    observer.receive_data(subject_comp, &asset_server, subject)
                }
            }
        }
    }
}

pub trait ObserverBuildCommandExt {
    /// Sets the component O on this entity to observe component S on the source entities.
    fn set_observer<S: Component, O: Observer<S>>(&mut self, source: Vec<Entity>) -> &mut Self;
}

impl<'w, 's, 'a> ObserverBuildCommandExt for EntityCommands<'w, 's, 'a> {
    fn set_observer<S: Component, O: Observer<S>>(&mut self, sources: Vec<Entity>) -> &mut Self {
        let id = self.id();

        self.commands().add(SyncToDataCommand::<S, O> {
            observer: id,
            subjects: sources,
            phantom_subject: PhantomData,
            phantom_observer: PhantomData,
        });

        self
    }
}

impl<'w> ObserverBuildCommandExt for EntityMut<'w> {
    fn set_observer<S: Component, O: Observer<S>>(&mut self, sources: Vec<Entity>) -> &mut Self {
        let id = self.id();
        unsafe {
            let world = self.world_mut();
            SyncToDataCommand::<S, O> {
                observer: id,
                subjects: sources,
                phantom_subject: PhantomData,
                phantom_observer: PhantomData,
            }
            .write(world)
        }

        self
    }
}

/// Sends events to all observer systems of this subject component when mutated.
fn send_subject_event<S: Component>(
    query: Query<Entity, Changed<S>>,
    mut event_writer: EventWriter<SubjectUpdateEvent<S>>,
) {
    for entity in query.iter() {
        event_writer.send(SubjectUpdateEvent {
            sender: entity,
            phantom_data: PhantomData,
        })
    }
}

/// Receives subject events from subjects and updates any observer component in ObserverList.
fn recieve_subject_event<S: Component, O: Observer<S>>(
    mut event_reader: EventReader<SubjectUpdateEvent<S>>,
    asset_server: Res<AssetServer>,
    mut observer_query: Query<&mut O>,
    mut observer_list_query: Query<(Entity, &S, &mut ObserverList<S, O>)>,
) {
    for event in event_reader.iter() {
        if let Ok((subject, subject_comp, mut observer_list)) =
            observer_list_query.get_mut(event.sender)
        {
            let mut remove_list = Vec::<Entity>::new();
            for &observer in observer_list.observers.iter() {
                match observer_query.get_mut(observer) {
                    Ok(mut observer) => {
                        observer.receive_data(subject_comp, &asset_server, subject);
                    }
                    Err(QueryEntityError::NoSuchEntity { .. }) => remove_list.push(observer),
                    _ => (),
                }
            }

            observer_list.observers.retain(|x| !remove_list.contains(x));
        }
    }
}

pub trait ObserverRegisterExt {
    /// Registers a type as capable to be observed.
    fn register_subject<S: Component>(&mut self) -> &mut Self;

    /// Register a type as capable of observing.
    fn register_observer<S: Component, O: Observer<S>>(&mut self) -> &mut Self;
}

impl ObserverRegisterExt for App {
    fn register_subject<S: Component>(&mut self) -> &mut Self {
        self.add_event::<SubjectUpdateEvent<S>>()
            .add_system_to_stage(
                CoreStage::PostUpdate,
                send_subject_event::<S>.label("SubjectUpdate"),
            );
        self
    }

    fn register_observer<S: Component, O: Observer<S>>(&mut self) -> &mut Self {
        self.register_type::<ObserverList<S, O>>()
            .add_system_to_stage(
                CoreStage::PostUpdate,
                recieve_subject_event::<S, O>.after("SubjectUpdate"),
            );
        self
    }
}

#[derive(Debug, Clone)]
pub struct SubjectUpdateEvent<S: Component> {
    sender: Entity,
    phantom_data: PhantomData<S>,
}

#[cfg(test)]
mod tests {
    use bevy::{asset::create_platform_default_asset_io, prelude::*, tasks::TaskPool};

    use crate::{Observer, ObserverBuildCommandExt, ObserverRegisterExt};

    #[derive(Component)]
    struct TestSubject {
        a: String,
        b: u32,
    }

    #[derive(Component, Default)]
    struct TestObserver {
        a: Option<String>,
        b: Option<u32>,
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
            .register_subject::<TestSubject>()
            .register_observer::<TestSubject, TestObserver>()
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
            .set_observer::<TestSubject, TestObserver>(vec![g])
            .id();

        app.update();

        assert_eq!(
            app.world.get::<TestObserver>(r).unwrap().a,
            Some("Farewell World!".to_string())
        );
        assert_eq!(app.world.get::<TestObserver>(r).unwrap().b, Some(12));
    }
}
