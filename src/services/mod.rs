use std::any::TypeId;

use crate::{dep_map::ServiceMap, Config};

pub mod beam;
pub mod focus;

pub trait Service: ToCompose {
    type Inputs: Service + 'static;

    fn from_config(conf: &Config, inputs: &mut Self::Inputs) -> Self
    where
        Self: Sized;

    fn deps() -> Vec<TypeId>
    where
        Self: Sized,
    {
        <Self::Inputs>::deps()
    }
}

impl Service for () {
    type Inputs = ();

    fn from_config(_conf: &Config, _inputs: &mut Self::Inputs) -> Self {
        ()
    }

    fn deps() -> Vec<TypeId> {
        Vec::with_capacity(0)
    }
}

impl ToCompose for () {
    fn to_compose(&self) -> serde_yaml::Value {
        serde_yaml::Value::Null
    }
}

// TODO: Think of something better for the tuples
impl<T1: ToCompose, T2: ToCompose> ToCompose for (T1, T2) {
    fn to_compose(&self) -> serde_yaml::Value {
        serde_yaml::Value::Sequence(vec![self.0.to_compose(), self.1.to_compose()])
    }
}

impl<T, T2> Service for (T, T2)
where
    T: Service,
    T::Inputs: Service,
    T2: Service,
    T2::Inputs: Service,
{
    type Inputs = (T::Inputs, T2::Inputs);

    fn from_config(conf: &Config, inputs: &mut Self::Inputs) -> Self {
        (
            T::from_config(conf, &mut inputs.0),
            T2::from_config(conf, &mut inputs.1),
        )
    }

    fn deps() -> Vec<TypeId> {
        let mut d = T::deps();
        d.extend(T2::deps());
        d
    }
}

trait ServiceMaker {
    fn make(conf: &Config, deps: &mut ServiceMap) -> Self;
}

// TODO: Maybe make this part of the Service trait and on the tuple impls override to not polute map
// nevermind does not work because of the lookup maybe we can mark them some other way tho?
impl<T: Service> ServiceMaker for T {
    fn make(conf: &Config, deps: &mut ServiceMap) -> Self {
        if let Some(inputs) = deps.get_mut() {
            T::from_config(conf, inputs)
        } else {
            let inputs = T::Inputs::make(conf, deps);
            deps.insert(inputs);
            T::from_config(conf, deps.get_mut().unwrap())
        }
    }
}

pub fn make_services<T: Service + 'static>(conf: &Config) -> ServiceMap {
    let mut services = ServiceMap::default();
    services.insert(());
    let t = T::make(conf, &mut services);
    services.insert(t);
    services.remove::<()>();
    services
}

// impl<O> Service for dyn Fn(&Config) -> O {
//     type Inputs = ();
//     type Output = O;
//
//     fn from_config(
//         &self,
//         conf: &Config,
//         _inputs: &mut <Self::Inputs as Service>::Output,
//     ) -> Self::Output {
//         self(conf)
//     }
// }
//
// impl<O, A1> Service for dyn Fn(&Config, &mut A1::Output, PhantomData<A1>) -> O
// where
//     A1: Service,
// {
//     type Inputs = A1;
//     type Output = O;
//
//     fn from_config(
//         &self,
//         conf: &Config,
//         inputs: &mut <Self::Inputs as Service>::Output,
//     ) -> Self::Output {
//         self(conf, inputs, PhantomData)
//     }
// }
//
// impl<O, A1, A2> Service
//     for dyn Fn(&Config, &mut A1::Output, &mut A2::Output, PhantomData<(A1, A2)>) -> O
// where
//     A1: Service,
//     A2: Service,
// {
//     type Inputs = (A1, A2);
//     type Output = O;
//
//     fn from_config(
//         &self,
//         conf: &Config,
//         inputs: &mut <Self::Inputs as Service>::Output,
//     ) -> Self::Output {
//         self(conf, &mut inputs.0, &mut inputs.1, PhantomData)
//     }
// }

// impl<T, D1, S> Service<(D1,)> for T
// where
//     T: Fn(&Config, D1) -> S,
//     D1: 'static,
//     S: ToCompose + 'static,
// {
//     type Output = S;
//
//     fn from_config(&self, conf: &Config, deps: &mut ServiceMap) -> S {
//         self(conf, deps.get_mut().unwrap())
//     }
//
//     fn dependecies(&self) -> Vec<TypeId> {
//         vec![std::any::TypeId::of::<D1>()]
//     }
// }
//
// impl<S: ToCompose + 'static> Service for fn(&Config) -> S {
//     fn from_config(&self, conf: &Config, _deps: &mut ServiceMap) -> Box<dyn ToCompose> {
//         Box::new(self(conf))
//     }
//
//     fn dependecies(&self) -> Vec<TypeId> {
//         Vec::new()
//     }
// }
//
pub trait ToCompose {
    // TODO: Always quote secrets but just replace this with jinja templates
    // Acutal issue is $ in pws
    fn to_compose(&self) -> serde_yaml::Value;
}

