use std::any::TypeId;

use crate::{dep_map::ServiceMap, Config};

pub mod beam;
pub mod focus;

pub type Deps<'a, T> = <<T as Service>::Inputs<'a> as ServiceTuple<'a>>::DepRefs;

// Could remove 'static bound by using dtolnay's typeid crate for the type map
pub trait Service: ToCompose + 'static {
    type Inputs<'s>: ServiceTuple<'s>;

    fn from_config(conf: &Config, deps: Deps<'_, Self>) -> Self;

    fn get_or_create<'services>(
        conf: &Config,
        deps: &'services mut ServiceMap,
    ) -> &'services mut Self
    where
        Self: Sized,
    {
        // Workaround for problem case #3
        // https://smallcultfollowing.com/babysteps/blog/2016/04/27/non-lexical-lifetimes-introduction/
        if deps.contains::<Self>() {
            return deps.get_mut().unwrap();
        }
        let this = Self::from_config(conf, Self::Inputs::get_or_create(conf, deps));
        deps.insert(this);
        deps.get_mut().unwrap()
    }
}

pub trait ServiceTuple<'t> {
    type DepRefs;

    fn get_or_create<'service: 't>(
        conf: &Config,
        services: &'service mut ServiceMap,
    ) -> Self::DepRefs;
}

impl<'t> ServiceTuple<'t> for () {
    type DepRefs = ();

    fn get_or_create<'service: 't>(_conf: &Config, _services: &'service mut ServiceMap) -> Self {
        ()
    }
}

impl<'t, T: Service> ServiceTuple<'t> for (T,) {
    type DepRefs = (&'t mut T,);

    fn get_or_create<'service: 't>(
        conf: &Config,
        services: &'service mut ServiceMap,
    ) -> Self::DepRefs {
        (T::get_or_create(conf, services),)
    }
}

impl<'t, T1: Service, T2: Service> ServiceTuple<'t> for (T1, T2) {
    type DepRefs = (&'t mut T1, &'t mut T2);

    fn get_or_create<'service: 't>(
        conf: &Config,
        services: &'service mut ServiceMap,
    ) -> Self::DepRefs {
        assert_ne!(TypeId::of::<T1>(), TypeId::of::<T2>());
        // Safety:
        // This is basically a HashMap::get_many_mut so as long as they don't overlap this code is
        // sound
        unsafe {
            (
                T1::get_or_create(conf, &mut *(services as *mut _)),
                T2::get_or_create(conf, &mut *(services as *mut _)),
            )
        }
    }
}

pub trait ToCompose {
    // TODO: Always quote secrets but just replace this with jinja templates
    // Acutal issue is $ in pws
    fn to_compose(&self) -> serde_yaml::Value;
}
