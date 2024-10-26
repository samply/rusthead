use crate::{dep_map::ServiceMap, Config};

pub mod beam;
pub mod focus;

pub trait Service: ToCompose + 'static {
    type Inputs: Service + 'static;

    fn from_config(conf: &Config, inputs: &mut Self::Inputs) -> Self
    where
        Self: Sized;

    fn make<'services>(conf: &Config, deps: &'services mut ServiceMap) -> &'services mut Self
    where
        Self: Sized,
    {
        // Workaround for problem case #3
        // https://smallcultfollowing.com/babysteps/blog/2016/04/27/non-lexical-lifetimes-introduction/
        if deps.contains::<Self>() {
            return deps.get_mut().unwrap();
        }
        let this = Self::from_config(conf, Self::Inputs::make(conf, deps));
        deps.insert(this);
        deps.get_mut().unwrap()
    }
}

impl Service for () {
    type Inputs = ();

    fn from_config(_conf: &Config, _inputs: &mut Self::Inputs) -> Self {
        ()
    }

    fn make<'a>(_conf: &Config, _deps: &'a mut ServiceMap) -> &'a mut Self
    where
        Self: Sized,
    {
        static mut THIS: () = ();
        #[allow(static_mut_refs)]
        unsafe {
            &mut THIS
        }
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
}

pub fn make_services<T: Service + 'static>(conf: &Config) -> ServiceMap {
    let mut services = ServiceMap::default();
    T::make(conf, &mut services);
    services
}

pub trait ToCompose {
    // TODO: Always quote secrets but just replace this with jinja templates
    // Acutal issue is $ in pws
    fn to_compose(&self) -> serde_yaml::Value;
}
