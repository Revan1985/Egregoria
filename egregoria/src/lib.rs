use crate::economy::{Bought, Sold, Workers};
use crate::engine_interaction::{Selectable, WorldCommands};
use crate::map_dynamic::{Itinerary, Router};
use crate::pedestrians::Pedestrian;
use crate::physics::CollisionWorld;
use crate::physics::{Collider, Kinematics};
use crate::souls::add_souls_to_empty_buildings;
use crate::souls::desire::{BuyFood, Home, Work};
use crate::souls::goods_company::GoodsCompany;
use crate::souls::human::HumanDecision;
use crate::vehicles::Vehicle;
use common::saveload::Encoder;
use geom::{Transform, Vec3};
use hecs::{Component, Entity, EntityBuilder, EntityRef, World};
use map_model::Map;
use pedestrians::Location;
use resources::{Ref, RefMut, Resource, Resources};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::hash::Hash;
use std::num::NonZeroU64;
use std::time::{Duration, Instant};
use utils::rand_provider::RandProvider;
use utils::scheduler::SeqSchedule;
use utils::time::{GameTime, SECONDS_PER_DAY, SECONDS_PER_HOUR};

#[macro_use]
extern crate common;

#[macro_use]
extern crate imgui_inspect;

#[macro_use]
extern crate log as extern_log;

pub mod economy;
pub mod engine_interaction;
pub mod init;
pub mod map_dynamic;
pub mod pedestrians;
pub mod physics;
pub mod souls;
mod tests;
pub mod utils;
pub mod vehicles;

use crate::init::{GSYSTEMS, INIT_FUNCS, SAVELOAD_FUNCS};
use crate::utils::scheduler::RunnableSystem;
use common::FastMap;
use serde::de::Error;
pub use utils::par_command_buffer::ParCommandBuffer;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[repr(transparent)]
pub struct SoulID(pub Entity);

impl PartialOrd for SoulID {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let sel: NonZeroU64 = unsafe { std::mem::transmute(self.0) };
        let other: NonZeroU64 = unsafe { std::mem::transmute(other.0) };
        sel.partial_cmp(&other)
    }
}

impl Ord for SoulID {
    fn cmp(&self, other: &Self) -> Ordering {
        let sel: NonZeroU64 = unsafe { std::mem::transmute(self.0) };
        let other: NonZeroU64 = unsafe { std::mem::transmute(other.0) };
        sel.cmp(&other)
    }
}

debug_inspect_impl!(SoulID);

pub struct Egregoria {
    pub(crate) world: World,
    resources: Resources,
    tick: u32,
}

/// Safety: Resources must be Send+Sync.
/// Guaranteed by `Egregoria::insert`.
/// World is Send+Sync and `SeqSchedule` too
unsafe impl Sync for Egregoria {}

const RNG_SEED: u64 = 123;

impl Egregoria {
    pub fn schedule() -> SeqSchedule {
        let mut schedule = SeqSchedule::default();
        unsafe {
            for s in &GSYSTEMS {
                let s = (s.s)();
                schedule.add_system(s);
            }
        }
        schedule
    }

    pub fn new(size: i32) -> Egregoria {
        let mut goria = Egregoria {
            world: Default::default(),
            resources: Default::default(),
            tick: 0,
        };

        info!("Seed is {}", RNG_SEED);

        unsafe {
            for s in &INIT_FUNCS {
                (s.f)(&mut goria);
            }
        }

        for y in -size + 1..size {
            for x in -size + 1..size {
                goria.write::<Map>().terrain.generate_chunk((x, y));
            }
        }

        goria
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    pub fn tick(&mut self, game_schedule: &mut SeqSchedule, commands: &WorldCommands) -> Duration {
        self.tick += 1;
        const WORLD_TICK_DT: f32 = 0.05;

        let t = Instant::now();

        {
            let mut time = self.write::<GameTime>();
            *time = GameTime::new(WORLD_TICK_DT, time.timestamp + WORLD_TICK_DT as f64);
        }

        for command in &commands.commands {
            command.apply(self);
        }

        game_schedule.execute(self);
        add_souls_to_empty_buildings(self);
        t.elapsed()
    }

    pub fn get_tick(&self) -> u32 {
        self.tick
    }

    pub fn hashes(&self) -> BTreeMap<String, u64> {
        let mut hashes = BTreeMap::new();
        hashes.insert("tick".to_string(), self.tick as u64);
        let ser = common::saveload::Bincode::encode(&SerWorld(&self.world)).unwrap();
        hashes.insert("world".to_string(), common::hash_u64(&*ser));

        unsafe {
            for l in &SAVELOAD_FUNCS {
                let v = (l.save)(self);
                hashes.insert(l.name.to_string(), common::hash_u64(&*v));
            }
        }

        hashes
    }

    pub fn load_from_disk(save_name: &'static str) -> Option<Self> {
        let goria: Egregoria = common::saveload::JSON::load(save_name)?;
        Some(goria)
    }

    pub fn save_to_disk(&self, save_name: &'static str) {
        common::saveload::JSON::save(&self, save_name);
    }

    pub fn pos(&self, e: Entity) -> Option<Vec3> {
        self.comp::<Transform>(e).map(|x| x.position)
    }

    pub(crate) fn add_comp(&mut self, e: Entity, c: impl Component) {
        if self.world.insert_one(e, c).is_err() {
            log::error!("trying to add component to entity but it doesn't exist");
        }
    }
    pub fn comptest<T: Component>(&self, e: Entity) -> Option<&T> {
        match self.world.get::<&T>(e).ok() {
            None => None,
            Some(x) => Some(*x),
        }
    }

    pub fn comp<T: Component>(&self, e: Entity) -> Option<hecs::Ref<T>> {
        self.world.get(e).ok()
    }

    pub fn comp_mut<T: Component>(&mut self, e: Entity) -> Option<hecs::RefMut<T>> {
        self.world.get_mut(e).ok()
    }

    pub fn write_or_default<T: Resource + Default>(&mut self) -> RefMut<T> {
        self.resources.entry::<T>().or_default()
    }

    pub fn try_write<T: Resource>(&self) -> Option<RefMut<T>> {
        self.resources.get_mut().ok()
    }

    pub fn write<T: Resource>(&self) -> RefMut<T> {
        self.resources
            .get_mut()
            .unwrap_or_else(|_| panic!("Couldn't fetch resource {}", std::any::type_name::<T>()))
    }

    pub fn read<T: Resource>(&self) -> Ref<T> {
        self.resources
            .get()
            .unwrap_or_else(|_| panic!("Couldn't fetch resource {}", std::any::type_name::<T>()))
    }

    pub fn map(&self) -> Ref<'_, Map> {
        self.resources.get().unwrap()
    }

    pub(crate) fn map_mut(&self) -> RefMut<'_, Map> {
        self.resources.get_mut().unwrap()
    }

    pub fn insert<T: Resource>(&mut self, res: T) {
        self.resources.insert(res);
    }
}

struct SerWorld<'a>(&'a World);

impl<'a> Serialize for SerWorld<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        hecs::serialize::row::serialize(self.0, &mut SerContext, serializer)
    }
}

impl Serialize for Egregoria {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut m: FastMap<String, Vec<u8>> = FastMap::default();

        unsafe {
            for l in &SAVELOAD_FUNCS {
                let v: Vec<u8> = (l.save)(self);
                m.insert(l.name.to_string(), v);
            }
        }

        EgregoriaSer {
            world: SerWorld(&self.world),
            version: goria_version::VERSION.to_string(),
            res: m,
            tick: self.tick,
        }
        .serialize(serializer)
    }
}

#[derive(Serialize)]
struct EgregoriaSer<'a> {
    world: SerWorld<'a>,
    version: String,
    res: FastMap<String, Vec<u8>>,
    tick: u32,
}

#[derive(Deserialize)]
struct EgregoriaDeser {
    world: DeserWorld,
    version: String,
    res: FastMap<String, Vec<u8>>,
    tick: u32,
}

impl<'de> Deserialize<'de> for Egregoria {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut goriadeser = EgregoriaDeser::deserialize(deserializer)?;

        if goriadeser.version != goria_version::VERSION {
            return Err(Error::custom(format!(
                "couldn't load save, incompatible version! save is: {} - game is: {}",
                goriadeser.version,
                goria_version::VERSION
            )));
        }

        let mut goria = Self::new(0);

        goria.world = goriadeser.world.0;
        goria.tick = goriadeser.tick;

        unsafe {
            for l in &SAVELOAD_FUNCS {
                if let Some(data) = goriadeser.res.remove(l.name) {
                    (l.load)(&mut goria, data);
                }
            }
        }

        Ok(goria)
    }
}

struct DeserWorld(World);

impl<'de> Deserialize<'de> for DeserWorld {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        hecs::serialize::row::deserialize(&mut DeserContext, deserializer).map(DeserWorld)
    }
}

struct SerContext;
struct DeserContext;

macro_rules! register {
    ($($t: ty => $p:literal),+,) => {
        impl hecs::serialize::row::SerializeContext for SerContext {
            fn serialize_entity<S>(
                &mut self,
                entity: EntityRef<'_>,
                map: &mut S,
            ) -> Result<(), S::Error>
            where
                S: serde::ser::SerializeMap,
            {
                $(
                    hecs::serialize::row::try_serialize::<$t, _, _>(&entity, &$p, map)?;
                )+
                Ok(())
            }
        }

        impl hecs::serialize::row::DeserializeContext for DeserContext {
            fn deserialize_entity<'de, M>(
                &mut self,
                mut map: M,
                entity: &mut EntityBuilder,
            ) -> Result<(), M::Error>
                where
                    M: serde::de::MapAccess<'de>,
            {
                while let Some(key) = map.next_key::<u64>()? {
                    match key {
                        $(
                           $p => {entity.add::<$t>(map.next_value()?);},
                        )+
                        _ => continue,
                    }
                }
                Ok(())
            }
        }
    };
}

pub struct NoSerialize;

register!(
        Transform => 0,
        Bought => 1,
        BuyFood => 2,
        Collider => 3,
        GoodsCompany => 4,
        Home => 5,
        HumanDecision => 6,
        Itinerary => 7,
        Kinematics => 8,
        Location => 9,
        Pedestrian => 10,
        Router => 11,
        Selectable => 12,
        Sold => 13,
        Vehicle => 14,
        Work => 15,
        Workers => 16,
);
